# Contract: Tauri commands and composable surface

The only external interfaces this feature changes are the Tauri command surface between the Vue
frontend and the Rust backend, and the public shape of two frontend composables. All wire structs
use `#[serde(rename_all = "camelCase")]`; the embedded `ModelCapabilities` follows the same
convention (see `capabilities-json.md`).

## Removed

| Command                                       | Replaced by                                                                                                                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_effort_levels(modelId) -> EffortLevel[]` | The `capabilities` field already present on `list_installed_models` / `list_provider_models` rows. **No replacement command** — a second IPC path would duplicate the cache. |

`lib.rs` drops the `get_effort_levels` import and registration. Frontend: `useChat().getEffortLevelsAsync`
is deleted.

## Changed

### `send_message(args)`

| Field             | Before                                                    | After                                         |
| ----------------- | --------------------------------------------------------- | --------------------------------------------- |
| `effortLevel`     | `"low" \| "medium" \| "high" \| "xhigh" \| "max" \| null` | removed                                       |
| `reasoningOption` | —                                                         | `string \| null`, a provider-native option id |

Semantics: `null` or omitted → Auto. The backend reads the cached model row once and keeps the id
only if it is one of that model's current `Presets` options; otherwise it is dropped and the model's
default applies (FR-013). A dropped id is **not** an error and is not reported. The option is
transient request input and is never persisted with the message (unchanged from today).

### `inspect_attachment(path, modelId)`

Signature unchanged. Behavior: no `Provider` lookup and no `(ProviderKind, adapter)` match; it reads
the selected `ModelRow.capabilities` once.

| Capabilities state                                                  | Result                                                                          |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `accepted_attachment_kinds = Some(list)` containing the file's kind | `usable` unchanged from file checks                                             |
| `Some(list)` not containing it                                      | `usable: false`, `reason: "not usable by the selected model"` (existing text)   |
| `None` (not determined) or no row                                   | `usable: false`, `reason: "attachment support for this model is not yet known"` |

Size/type reasons from file classification keep their existing precedence behavior.

### `list_installed_models()` → `InstalledModelPayload[]`

Adds `capabilities: ModelCapabilities | null`. Side effect (idempotent): runs
`backfill_local_capabilities` alongside the existing `backfill_*` calls.

### `list_provider_models(providerId)` → `ProviderModelPayload[]`

Adds `capabilities: ModelCapabilities | null`. Still a cached read; never triggers a fetch.

### `refresh_provider_models(providerId)` / `add_provider`

Signature unchanged. Behavior: the refreshed rows now carry and persist capabilities (overwrite, not
merge); a failed refresh leaves stored capabilities untouched (existing cache-preserving behavior).

## Unchanged

`load_model`, `resolve_default_model`, `get_pref`, `set_pref`, `clear_pref` (used for the effort
preference through the existing `usePreferences()` API), `current_device_info`.

## Frontend composable / store surface

| Symbol                                 | Change                                                                                                                             |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `useChat().sendMessageAsync(args)`     | `args.effortLevel` → `args.reasoningOption: string \| null`                                                                        |
| `useChat().getEffortLevelsAsync`       | removed                                                                                                                            |
| `useProviders().refreshModelsAsync`    | unchanged; gains its first caller (FR-022)                                                                                         |
| `useModelsStore()`                     | adds `displayModelCapabilities`, `effortState`, `effortLevel`, `updateEffortLevel`; existing exports keep their names and behavior |
| `InstalledModel`, `ProviderModel` (TS) | gain `capabilities: ModelCapabilities \| null`                                                                                     |
| `ComposerSettingsPopover` props        | `effortLevels: string[]` + `effortLevel: string \| null` + new `effortState`; still plain props                                    |

## Settings UI contract (FR-022)

`ConnectDelegateProvider.vue`: for each connected provider, a "Refresh models" button calling
`refreshModelsAsync(provider.id)`. States: idle, in-flight (button disabled, spinner), success
(brief confirmation), error (message via `useErrorString`, previous capabilities untouched). New
i18n keys in `en.json` and `de.json`.
