# holzi

Portable personal-agent Tauri app. One user, one operator, small set of
their own devices sharing an encrypted SQLite vault via CRDT sync.

## Language

### Identity model

**Vault**:
The encrypted SQLite database file (`*.db`) plus its schema. The unit
of ownership: one vault holds one operator's data.
_Avoid_: instance (used to mean this earlier, deprecated), database.

**Installation UUID**:
Per-app-installation identifier stored in a file at
`<AppLocalData>/installation-id`. The corresponding `installation_uuid` is
also recorded in the sync-tracked `known_devices` table, so a copied vault
can carry that historical row with it. The lookup key `HolziBootstrap` uses to
decide whether the current open is genesis, resume, or adoption of a copied
vault; adoption mints a fresh `vault_device_uuid` for the new installation.
_Avoid_: device id (ambiguous — see below).

**Vault Device UUID**:
Per-`(vault × installation)` identifier stored in `known_devices`. The
CRDT `device_id` for HLC timestamps and signatures on this replica.
Different on every device that has ever opened this vault, including a
device that opened a copy of the vault (the adoption path mints a new
one). Used as the FK target for per-device rows.
_Avoid_: device_id (unqualified — usually means this, but callers
sometimes mean installation_uuid).

**Vault Identity Keypair**:
Singleton `vault_identity` row containing the vault's secp256k1
keypair. Copies of the vault share the same keypair — this is the
identity the vault itself holds and moves with the file.

**Adoption**:
The `HolziBootstrap` path taken when a copied vault file is opened on
a device that has never seen it. Mints a fresh `vault_device_uuid`,
which automatically isolates the new device from the source device's
per-device rows.

### Scope conventions

**Per-vault data**:
Rows that apply to the whole vault across every device. Examples:
`providers`, `models`, `chat_threads`, `chat_messages`,
`vault_preferences` entries (see below). Sync-tracked, no
device-discriminator column.

**Per-device data**:
Rows that apply only to a specific device. Composite PK includes
`vault_device_uuid` with FK to `known_devices`. See ADR-0001.

**Vault Scope Sentinel**:
The nil UUID `00000000-0000-0000-0000-000000000000` — reserved as a
row in `known_devices` representing "vault-wide" in tables that would
otherwise be device-scoped. See ADR-0001.

### Model catalog

**Catalog entry**:
One of the compiled-in GGUF suggestions under
`src-tauri/src/catalog/model_catalog.json`. Not installed until the
user downloads it.

**Installed model**:
A GGUF file physically present at
`<AppLocalData>/models/<slug>/<filename>` on the current device. The
filesystem is the authority for "installed on this device"; the
`models` table only carries catalog metadata (name, context window,
tokenizer repo, provider id).

**Provider**:
An entry in `providers`: `local` (mistralrs in-process), `api_key`
(external HTTP API such as Anthropic), or `cli_delegate` (spawn an
external CLI — design pass open).

**Model ID**:
Cross-provider unique string. For local models: catalog id. For api_key
models: `<provider_uuid>:<remote_model_id>` composite. Persisted verbatim
as `chat_messages.model_id`.

**Fit**:
Hardware verdict for a catalog entry — `Fits` / `Tight` / `TooBig` /
`Unknown`. See `hardware::fit`.

### Chat runtime

**Adapter**:
`ProviderAdapter` trait implementation. One per provider kind:
`LocalAdapter` wraps a loaded `LocalModel`; `AnthropicAdapter` wraps
the HTTP client. `ChatState` holds `Arc<dyn ProviderAdapter>`.

**Active Session**:
The currently-loaded model for chatting. In-memory in `ChatState`,
not persisted directly. Recovered on next launch via the session
resolver reading `preferences`.

**Session Resolver**:
The chain `last_active_model → default_model (device) → default_model
(vault) → first-available → onboarding` that decides which model
loads on chat-page mount. Only the terminal load runs — no
preference-write happens along this chain.

### Sprachkonventionen im UI

**Workspace / Arbeitsbereich**:
Same concept. "Arbeitsbereich" as German UI label (visible in
onboarding wizard, workspace-landing headers). "Workspace" in code,
folder names (`src/pages/workspace/`), and technical documentation.

**Standard-Modell / `default_model_id`**:
Same value. "Standard-Modell" as German UI label. `default_model_id`
as the preferences-key suffix and Rust/TypeScript identifier.

**Gerätename / `alias`**:
Same value. "Gerätename" as German UI label. `alias` as the
`known_devices` column and Rust/TypeScript identifier.

### Internationalisierung (i18n)

All user-visible text uses `@nuxtjs/i18n`. Backend commands and
events return structured data (enum values, IDs, parameters), never
localized strings. Locale files under `src/i18n/de/*.json` and
`src/i18n/en/*.json` — both languages MUST be maintained in lockstep
for every user-visible string introduced by a feature.
_Avoid_: hardcoded German strings in `.vue` files or in backend
event payloads.
