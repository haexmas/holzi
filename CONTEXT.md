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

**Window Manager (`wm`) / App / Fenster (Window) / Tab / Launcher** (spec 015):
The in-app desktop concept, adopted from haex-vault. A **Workspace**
(Arbeitsbereich) contains **Windows** (Fenster); each Window contains
one or more **Tabs**, each Tab an instance of an **App** (Chat,
Einstellungen; Föderation was an App in spec 015 and is a settings category
since spec 023). The **Launcher** lists
available Apps and opens them as Windows. "Fenster" and "Tab" as
German UI labels; `wm` as the code prefix (`useWindowManagerStore`,
`src/lib/wm/`, `src/components/wm/`, action ids `wm.*`, Tauri commands
`wm_*`) and "window"/"tab"/"app" as identifiers (`WmWindow`, `WmTab`,
`AppDefinition`). Not to be confused with the OS-level application window
(singular, Tauri-managed); the window manager is an in-app desktop rendered
inside it. Specs 015 and 020 call it **Shell**; it was renamed on
2026-09-25 because "shell" also means the command line, which agents will
be granted or denied access to (spec 021). Do not use "shell" for this
concept in new code or specs.

**Sitzung (wm session)** (spec 022):
Which workspaces, windows and tabs are open, with window geometry and state and
each tab's location and back/forward history. Saved (`wm_sessions_no_sync`, one
JSON row per device, never synced) only while the setting "Sitzung
wiederherstellen" (`wm.session_restore`, one value for the vault since spec 023,
off by default) applies; turning it off deletes the saved session. Not the **vault
session** (spec 013: unlock to lock, one process) and not the chat's **Active
Session** (the loaded model). Avoid "Layout" for this in user-facing text.

**Ort (Location) / Tab-Historie** (spec 020):
Where a Tab stands inside its App: an app-relative path plus query
(`/thread/<id>`, `/models?sort=size`), pure data (`TabLocation`). Each
Tab keeps its own linear back/forward **Tab-Historie** (`TabHistory`),
in memory only. "Ort" and "Verlauf" as German UI wording; never the
webview's browser history, which holzi does not use as navigation state.

**Aktion (Action) / Berechtigungsbereich (Scope) / Aufrufer (Caller)**
(spec 020):
Every state-changing control of the window manager and its Apps triggers a
catalog **Aktion** (`ActionDefinition`, `runAction`) with a JSON
schema, a target, a **Berechtigungsbereich** and an effect; the
**Aufrufer** is the user, the built-in agent or an external agent.
Actions in the `guardrails` scope are user-only.
_Avoid_: "command" for these — in holzi "command" means Tauri commands.

**Werkzeug (tool) / Wirkungsart / Risikostufe** (spec 032):
A **Werkzeug** is an **Aktion** offered to a model that holzi's own chat
drives, local or reached with an API key; the tool loop lists it with source
`action`. A chat turn starts with a fixed core of tools and the search
`find_actions`; the model finds the rest and gets it from the next step on. The
**Wirkungsart** of an action (`read`, `write`, `destructive`) maps to the
**Risikostufe** the approval mode decides on: `Safe`, `Change`, `Risky`.
Actions in the `guardrails` scope are never offered. Whether a model can call
tools at all is its capability `tool_use` (`ModelCapabilities`).
_Avoid_: "command" for tools — in holzi "command" means Tauri commands.

**Einstellungskategorie (settings category) / Unteransicht** (spec 023):
A group in the settings' sidebar — Allgemein, Darstellung, Modelle, Agenten,
Föderation (`SETTINGS_CATEGORIES` in `src/lib/settings/registry.ts`). A
category with several areas starts with an overview that leads into
**Unteransichten**; every category and sub-view is an **Ort** of the settings
tab. Settings apply to the vault on every device; only the Gerätename, the
Standard-Modell and the Spracherkennungsmodell stay per device (FR-024). A
choice saves on selection, without a save button.

**Farbschema (color scheme)** (spec 023):
Hell, Dunkel or System (`appearance.color_scheme`, vault value, default
System); System follows the operating system. Applied as the `dark` class and
`color-scheme` on `<html>`. Pages before unlocking follow the operating system.
_Avoid_: "Theme" in user-facing text.

**Geräte der Vault (vault devices)** (spec 023):
The rows of `known_devices` except the Vault Scope Sentinel, shown in the
category "Föderation" (`list_vault_devices`, action `settings.devices.list`):
this device first and marked "Dieses Gerät", the others by Gerätename, unnamed
ones as "Unbenanntes Gerät". Other devices appear once the sync exists or in a
copied vault; "zuletzt online" waits for the sync spec.

### Sync zwischen eigenen Geräten (spec 024)

**Hauptgerät (main device)**:
A device of the vault that holds the private key of the vault identity. Only
main devices sign device lists, so only they add or remove devices. A vault can
have several; its first installation is one.

**Verknüpftes Gerät (linked device)**:
A device of the vault without the private key of the vault identity. It reads
and writes all vault data but cannot add or remove devices.

**Geräteliste (device list)**:
The device list signed with the vault identity: the current devices (device
key, role, name, network id) and the removed devices with their limit, under a
generation. The valid list is the one with the highest generation; on a tie the
one with the smallest hash. Devices accept each other only when both are on it.

**Ursprungsgerät (origin device)**:
The device that wrote a change. It is the node id in the change's HLC (its
`vault_device_uuid`) and stays the same when the change is passed on.
_Avoid_: author, sender (the sender of a change can be a third device).

**Fortschrittsstand (progress)**:
Per origin device, the highest HLC this device has fully applied. Two devices
compare their progress and pull what they are missing.

**Transaktionsgruppe (transaction group)**:
All changes that share one HLC, which is one `Database::write` transaction.
It is never split and never applied in part.

**Nostr-Relay / iroh-Relay / Sync-Server**:
Three different third parties: a Nostr-Relay carries presence messages and
link rendezvous, an iroh-Relay forwards encrypted connections between devices
that cannot reach each other directly, and the Sync-Server (spec 026) keeps a
mailbox for devices that are offline.
_Avoid_: "Relay" on its own.

### Internationalisierung (i18n)

All user-visible text uses `@nuxtjs/i18n`. Backend commands and
events return structured data (enum values, IDs, parameters), never
localized strings. Locale files under `src/i18n/locales/de.json` and
`src/i18n/locales/en.json` — both languages MUST be maintained in lockstep
for every user-visible string introduced by a feature.
_Avoid_: hardcoded German strings in `.vue` files or in backend
event payloads.
