//! Reference commands (spec 036, `contracts/tauri-commands.md`): marks of a text, the placeholder
//! for a value, the keys of custom fields and the usage of a source. Each calls only the service as
//! `Caller::User`; none returns a value.

use serde::Deserialize;
use tauri::State;
use ts_rs::TS;

use super::service;
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::model_references::{RefMark, RefMarkKind, ReferenceUsage};
use crate::state::AppState;

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ReferencesParseArgs {
    pub text: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ReferenceTokenArgs {
    pub item_id: String,
    pub kind: RefMarkKind,
    #[ts(optional)]
    pub key: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ItemKeyNamesArgs {
    pub item_id: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct ReferenceUsageArgs {
    pub item_ids: Vec<String>,
}

/// The placeholders of a text with source, title of the source and state.
#[tauri::command]
pub async fn passwords_references_parse(
    state: State<'_, AppState>,
    args: ReferencesParseArgs,
) -> Result<Vec<RefMark>> {
    service(&state)?
        .references_parse(&Caller::User, args.text)
        .await
}

/// The placeholder for a value of an entry.
#[tauri::command]
pub async fn passwords_reference_token(
    state: State<'_, AppState>,
    args: ReferenceTokenArgs,
) -> Result<String> {
    service(&state)?
        .reference_token(&Caller::User, args.item_id, args.kind, args.key)
        .await
}

/// The keys of an entry's custom fields, not their values.
#[tauri::command]
pub async fn passwords_item_key_names(
    state: State<'_, AppState>,
    args: ItemKeyNamesArgs,
) -> Result<Vec<String>> {
    service(&state)?
        .item_key_names(&Caller::User, args.item_id)
        .await
}

/// How many entries point at each of the given ones.
#[tauri::command]
pub async fn passwords_reference_usage(
    state: State<'_, AppState>,
    args: ReferenceUsageArgs,
) -> Result<Vec<ReferenceUsage>> {
    service(&state)?
        .reference_usage(&Caller::User, args.item_ids)
        .await
}
