//! An extension proposes, changes, tests and removes storages (spec 038 US3, FR-013, FR-013a,
//! FR-009b, contracts/bridge.md, research R6, R8, R11). Every change goes through a dialog of holzi;
//! credentials are typed only in holzi's window over the whole app and never pass the bridge: a
//! call with credentials is refused before any dialog. A proposed endpoint needs the permission
//! `remoteStorage`/`add` for its host, with which it may also be local; addresses holzi never
//! reaches are refused before the dialog too.

use std::time::{Duration, SystemTime};

use reqwest::Url;
use serde_json::{json, Map, Value};
use url::Host;

use super::bridge::blocking::block_on;
use super::bridge::dispatch::CallContext;
use super::commands::permissions::{set, PermissionSetArgs};
use super::error::{BridgeError, ExtensionErrorCode};
use super::permissions::{Action, RequestTarget};
use super::remote_storage::{
    check, check_storage, grants, invalid, item, not_found, overview, provider, service, text,
};
use super::remote_storage_dialog::{ask, DialogKind, StorageAnswer};
use crate::error::HolziError;
use crate::passwords::clock::unix_millis;
use crate::remote_storage::address::{self, AddressError};
use crate::remote_storage::model::{ConnectionInput, CredentialsInput, StorageInput};
use crate::remote_storage::service::StorageService;
use crate::remote_storage::{Addressing, ConnectionRow, EndpointScope, ProviderKind, TestOutcome};
use crate::storage::query::Query;

pub const MODULE: &str = module_path!();

/// Fields a call may never carry (FR-013a).
const CREDENTIAL_FIELDS: [&str; 3] = ["accessKeyId", "secretAccessKey", "sessionToken"];

/// How long holzi waits for the addresses of a proposed endpoint before the dialog.
const RESOLVE_TIME: Duration = Duration::from_secs(10);

fn cancelled() -> BridgeError {
    BridgeError::new(
        ExtensionErrorCode::PermissionDenied,
        "cancelled by the user",
    )
}

/// The extension's error for an error of the storage service; never its endpoint or region.
fn from_service(error: HolziError) -> BridgeError {
    match error {
        HolziError::StorageTestFailed { outcome, .. } => provider(kind_of(outcome)),
        HolziError::StorageInvalid { field } => invalid(&format!("invalid {field}")),
        HolziError::StorageNotFound => not_found(),
        HolziError::StorageCredentialsUnavailable { .. } => provider("accessDenied"),
        _ => BridgeError::new(ExtensionErrorCode::Database, "database unavailable"),
    }
}

/// The `kind` of a failed test for the extension (research R7).
fn kind_of(outcome: TestOutcome) -> &'static str {
    match outcome {
        TestOutcome::Passed => "passed",
        TestOutcome::AccessDenied | TestOutcome::MissingRight => "accessDenied",
        TestOutcome::Unreachable => "network",
        TestOutcome::BucketMissing => "bucketMissing",
    }
}

/// The request object of add and update.
fn request(params: &Value) -> Result<&Map<String, Value>, BridgeError> {
    params
        .get("request")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("request must be an object"))
}

/// `config`, refused when it carries credentials (FR-013a) or a field outside `allowed`.
fn config<'a>(
    request: &'a Map<String, Value>,
    allowed: &[&str],
) -> Result<Map<String, Value>, BridgeError> {
    let config = match request.get("config") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(config)) => config.clone(),
        Some(_) => return Err(invalid("config must be an object")),
    };
    if CREDENTIAL_FIELDS.iter().any(|f| config.contains_key(*f)) {
        return Err(invalid("credentials are entered in holzi"));
    }
    if let Some(field) = config.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(invalid(&format!("config.{field} is not allowed here")));
    }
    Ok(config)
}

fn optional_text<'a>(
    map: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, BridgeError> {
    match map.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(_) => Err(invalid(&format!("{name} must be a string"))),
    }
}

/// The calling extension's name as the user knows it (for the dialog).
fn extension_name(ctx: &CallContext) -> String {
    let id = ctx.session.extension_id.to_string();
    ctx.db
        .read_blocking(move |q| {
            q.query_row(
                "SELECT COALESCE(display_name, name) FROM extensions WHERE id = ?1 \
                 UNION ALL SELECT COALESCE(display_name, name) FROM dev_extensions_no_sync \
                 WHERE id = ?1",
                &[&id],
                |r| r.get::<_, String>(0),
            )
        })
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Gives the extension "read and write" on the storage it created, for every own device.
fn grant(ctx: &CallContext, storage_id: &str) -> Result<(), BridgeError> {
    set(
        &ctx.db,
        ctx.device,
        PermissionSetArgs {
            extension_id: ctx.session.extension_id.to_string(),
            kind: "remoteStorage".into(),
            action: "readWrite".into(),
            target: storage_id.to_owned(),
            status: "granted".into(),
            all_devices: true,
            replaces: None,
        },
        unix_millis(SystemTime::now()),
    )
    .map_err(|_| BridgeError::new(ExtensionErrorCode::Database, "database unavailable"))
}

/// The list item of `storage_id` after a change.
fn answer_item(service: &StorageService, storage_id: &str) -> Result<Value, BridgeError> {
    item(&overview(service)?, storage_id).ok_or_else(not_found)
}

/// What the user typed or chose in the answer of a confirmed dialog.
struct Confirmed {
    connection_id: Option<String>,
    credentials: Option<CredentialsInput>,
    name: Option<String>,
    bucket: Option<String>,
}

fn confirmed(answer: StorageAnswer) -> Result<Confirmed, BridgeError> {
    match answer {
        StorageAnswer::Confirm {
            connection_id,
            credentials,
            name,
            bucket,
        } => Ok(Confirmed {
            connection_id,
            credentials,
            name,
            bucket,
        }),
        StorageAnswer::Cancel => Err(cancelled()),
    }
}

/// A proposed endpoint, checked before the dialog (research R8, FR-009b).
struct Proposed {
    url: Url,
    scope: EndpointScope,
}

/// Checks a proposed endpoint: its form, the `add` permission for its host (1004/1002), and that
/// its addresses lie in one scope holzi reaches (`http` only to a local one).
fn proposed_endpoint(ctx: &CallContext, endpoint: &str) -> Result<Proposed, BridgeError> {
    let url = address::check_endpoint(endpoint).map_err(|_| invalid("endpoint not allowed"))?;
    let host = match url.host() {
        Some(Host::Domain(name)) => name.to_ascii_lowercase(),
        Some(Host::Ipv4(ip)) => ip.to_string(),
        Some(Host::Ipv6(ip)) => format!("[{ip}]"),
        None => return Err(invalid("endpoint not allowed")),
    };
    let port = url
        .port_or_known_default()
        .ok_or_else(|| invalid("endpoint not allowed"))?;
    let grants = grants(ctx)?;
    check(
        ctx,
        &grants,
        Action::Add,
        RequestTarget::Endpoint {
            host: host.clone(),
            port,
        },
        &format!("{host}:{port}"),
    )?;
    let resolver = ctx.host.storage.resolver();
    let scope = block_on(async {
        tokio::time::timeout(RESOLVE_TIME, address::scope_of(&url, resolver.as_ref())).await
    });
    match scope {
        Ok(Ok(scope)) => Ok(Proposed { url, scope }),
        Ok(Err(AddressError::Invalid | AddressError::NotAllowed)) => {
            Err(invalid("endpoint not allowed"))
        }
        Ok(Err(AddressError::Unresolved)) | Err(_) => Err(provider("network")),
    }
}

/// Whether `connection` talks to `url` in `region`: the user may pick it instead of new
/// credentials.
fn same_place(connection: &ConnectionRow, url: &Url, region: &str) -> bool {
    Url::parse(&connection.endpoint).is_ok_and(|own| own == *url) && connection.region == region
}

/// `{request: {name, type: "s3", config: {endpoint?, region?, bucket, pathStyle?},
/// sameProviderAs?}}` → the new storage as in the list.
pub fn add_backend(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let request = request(params)?;
    let name = optional_text(request, "name")?.ok_or_else(|| invalid("name must be a string"))?;
    if optional_text(request, "type")? != Some("s3") {
        return Err(invalid("type must be s3"));
    }
    let service = service(ctx);
    let storage_id = match optional_text(request, "sameProviderAs")? {
        Some(same) => add_on_same_provider(ctx, &service, request, name, same)?,
        None => add_with_endpoint(ctx, &service, request, name)?,
    };
    grant(ctx, &storage_id)?;
    answer_item(&service, &storage_id)
}

/// A new bucket on the connection of a storage the extension may read.
fn add_on_same_provider(
    ctx: &CallContext,
    service: &StorageService,
    request: &Map<String, Value>,
    name: &str,
    same: &str,
) -> Result<String, BridgeError> {
    let config = config(request, &["bucket"])?;
    let bucket =
        optional_text(&config, "bucket")?.ok_or_else(|| invalid("bucket must be a string"))?;
    check_storage(ctx, Action::Read, same)?;
    let base = block_on(service.storage(same)).map_err(from_service)?;
    let connection = block_on(service.connection(&base.connection_id)).map_err(from_service)?;
    let answer = confirmed(ask(
        ctx,
        DialogKind::Add,
        &extension_name(ctx),
        json!({
            "name": name,
            "bucket": bucket,
            "providerName": connection.provider_name,
            "endpoint": connection.endpoint,
            "region": connection.region,
            "scope": connection.endpoint_scope.as_str(),
            "connectionId": connection.id,
            "sameProvider": true,
        }),
        &[],
    )?)?;
    let saved = block_on(service.save_storage(StorageInput {
        id: None,
        connection_id: connection.id,
        name: answer.name.unwrap_or_else(|| name.to_owned()),
        bucket: answer.bucket.unwrap_or_else(|| bucket.to_owned()),
    }))
    .map_err(from_service)?;
    Ok(saved.id)
}

/// A new storage on a proposed endpoint (or AWS by region): an existing connection the user picks,
/// or a new one with credentials typed in holzi's window.
fn add_with_endpoint(
    ctx: &CallContext,
    service: &StorageService,
    request: &Map<String, Value>,
    name: &str,
) -> Result<String, BridgeError> {
    let config = config(request, &["endpoint", "region", "bucket", "pathStyle"])?;
    let bucket =
        optional_text(&config, "bucket")?.ok_or_else(|| invalid("bucket must be a string"))?;
    let region =
        optional_text(&config, "region")?.ok_or_else(|| invalid("region must be a string"))?;
    let path_style = match config.get("pathStyle") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(path)) => Some(*path),
        Some(_) => return Err(invalid("pathStyle must be a boolean")),
    };
    let endpoint = optional_text(&config, "endpoint")?.filter(|e| !e.trim().is_empty());
    let (kind, proposed) = match endpoint {
        Some(endpoint) => (ProviderKind::Other, Some(proposed_endpoint(ctx, endpoint)?)),
        None => (ProviderKind::Aws, None),
    };
    let addressing = match path_style.unwrap_or(proposed.is_some()) {
        true => Addressing::Path,
        false => Addressing::Virtual,
    };
    let endpoint_text = proposed
        .as_ref()
        .map_or(String::new(), |p| p.url.to_string());
    let connections = block_on(service.connections()).map_err(from_service)?;
    let reusable: Vec<Value> = proposed
        .as_ref()
        .map(|p| {
            connections
                .iter()
                .filter(|c| same_place(c, &p.url, region))
                .map(|c| json!({ "id": c.id, "providerName": c.provider_name }))
                .collect()
        })
        .unwrap_or_default();
    let provider_name = proposed
        .as_ref()
        .and_then(|p| p.url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "AWS".to_owned());
    let answer = confirmed(ask(
        ctx,
        DialogKind::Add,
        &extension_name(ctx),
        json!({
            "name": name,
            "bucket": bucket,
            "providerName": provider_name,
            "endpoint": endpoint_text,
            "region": region,
            "insecure": proposed.as_ref().is_some_and(|p| address::is_insecure(&p.url)),
            "scope": proposed.as_ref().map_or("public", |p| p.scope.as_str()),
            "connections": reusable,
            "sameProvider": false,
        }),
        &[],
    )?)?;
    let storage_name = answer.name.clone().unwrap_or_else(|| name.to_owned());
    let storage_bucket = answer.bucket.clone().unwrap_or_else(|| bucket.to_owned());
    if let Some(connection_id) = answer.connection_id {
        let chosen = connections
            .iter()
            .find(|c| c.id == connection_id)
            .filter(|c| {
                proposed
                    .as_ref()
                    .is_some_and(|p| same_place(c, &p.url, region))
            })
            .ok_or_else(|| invalid("connection does not fit the proposal"))?;
        let saved = block_on(service.save_storage(StorageInput {
            id: None,
            connection_id: chosen.id.clone(),
            name: storage_name,
            bucket: storage_bucket,
        }))
        .map_err(from_service)?;
        return Ok(saved.id);
    }
    let credentials = answer.credentials.ok_or_else(cancelled)?;
    let connection = block_on(service.save_connection(ConnectionInput {
        id: None,
        provider_name,
        provider_kind: kind,
        endpoint: Some(endpoint_text),
        region: region.to_owned(),
        addressing,
        credentials: Some(credentials),
        bucket_for_test: storage_bucket.clone(),
    }))
    .map_err(from_service)?;
    match block_on(service.save_storage(StorageInput {
        id: None,
        connection_id: connection.id.clone(),
        name: storage_name,
        bucket: storage_bucket,
    })) {
        Ok(saved) => Ok(saved.id),
        Err(error) => {
            // Nothing stays from a proposal that did not end in a storage.
            if let Err(cleanup) = block_on(service.remove_connection(&connection.id)) {
                log::warn!("remote storage: connection of a failed proposal stays: {cleanup}");
            }
            Err(from_service(error))
        }
    }
}

/// `{request: {backendId, name?, config?: {bucket?}}}`: a confirmed change; new credentials only
/// from holzi's window.
pub fn update_backend(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let request = request(params)?;
    let storage_id = text(params, "backendId")?;
    let config = config(request, &["bucket"])?;
    let name = optional_text(request, "name")?;
    let bucket = optional_text(&config, "bucket")?;
    check_storage(ctx, Action::ReadWrite, storage_id)?;
    let service = service(ctx);
    let storage = block_on(service.storage(storage_id)).map_err(from_service)?;
    let connection = block_on(service.connection(&storage.connection_id)).map_err(from_service)?;
    let answer = confirmed(ask(
        ctx,
        DialogKind::Update,
        &extension_name(ctx),
        json!({
            "storageId": storage.id,
            "currentName": storage.name,
            "currentBucket": storage.bucket,
            "name": name.unwrap_or(&storage.name),
            "bucket": bucket.unwrap_or(&storage.bucket),
            "providerName": connection.provider_name,
            "endpoint": connection.endpoint,
            "region": connection.region,
            "scope": connection.endpoint_scope.as_str(),
        }),
        &[],
    )?)?;
    let new_bucket = answer
        .bucket
        .unwrap_or_else(|| bucket.unwrap_or(&storage.bucket).to_owned());
    if let Some(credentials) = answer.credentials {
        block_on(service.save_connection(ConnectionInput {
            id: Some(connection.id.clone()),
            provider_name: connection.provider_name.clone(),
            provider_kind: connection.provider_kind,
            endpoint: Some(connection.endpoint.clone()),
            region: connection.region.clone(),
            addressing: connection.addressing,
            credentials: Some(credentials),
            bucket_for_test: new_bucket.clone(),
        }))
        .map_err(from_service)?;
    }
    block_on(
        service.save_storage(StorageInput {
            id: Some(storage.id.clone()),
            connection_id: storage.connection_id.clone(),
            name: answer
                .name
                .unwrap_or_else(|| name.unwrap_or(&storage.name).to_owned()),
            bucket: new_bucket,
        }),
    )
    .map_err(from_service)?;
    answer_item(&service, &storage.id)
}

/// `{backendId}`: a confirmed test; `null` when it passed, else 2002 with its `kind`.
pub fn test_backend(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let storage_id = text(params, "backendId")?;
    check_storage(ctx, Action::ReadWrite, storage_id)?;
    let service = service(ctx);
    let storage = block_on(service.storage(storage_id)).map_err(from_service)?;
    confirmed(ask(
        ctx,
        DialogKind::Test,
        &extension_name(ctx),
        json!({ "storageId": storage.id, "name": storage.name, "bucket": storage.bucket }),
        &[],
    )?)?;
    let result = block_on(service.test_storage(storage_id)).map_err(from_service)?;
    match result.outcome {
        TestOutcome::Passed => Ok(Value::Null),
        outcome => Err(provider(kind_of(outcome))),
    }
}

/// `{backendId}`: a confirmed removal; the dialog names other extensions that lose access.
pub fn remove_backend(ctx: &CallContext, params: &Value) -> Result<Value, BridgeError> {
    let storage_id = text(params, "backendId")?;
    check_storage(ctx, Action::ReadWrite, storage_id)?;
    let service = service(ctx);
    let storage = block_on(service.storage(storage_id)).map_err(from_service)?;
    let own = extension_name(ctx);
    let id = storage.id.clone();
    let others: Vec<String> = ctx
        .db
        .read_blocking(move |q| {
            crate::remote_storage::store::extensions_of(q, &id).map_err(Into::into)
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|name| *name != own)
        .collect();
    confirmed(ask(
        ctx,
        DialogKind::Remove,
        &own,
        json!({ "storageId": storage.id, "name": storage.name, "bucket": storage.bucket }),
        &others,
    )?)?;
    block_on(service.remove_storage(storage_id)).map_err(from_service)?;
    Ok(Value::Null)
}

#[cfg(test)]
#[path = "remote_storage_manage_tests.rs"]
mod tests;
