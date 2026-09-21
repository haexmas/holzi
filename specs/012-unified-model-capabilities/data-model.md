# Data Model: Unified, Cached Model Capabilities

## ModelCapabilities (new, `src-tauri/src/model_capabilities.rs`)

| Field                       | Type                          | Meaning                                                             |
| --------------------------- | ----------------------------- | ------------------------------------------------------------------- |
| `reasoning`                 | `Option<ReasoningControl>`    | `None` = not determined.                                            |
| `accepted_attachment_kinds` | `Option<Vec<AttachmentKind>>` | `None` = not determined; `Some([])` = authoritatively none.         |
| `thinking_style`            | `Option<ThinkingStyle>`       | Adapter-private hint (research R2); `None` for local/Codex/unknown. |

Derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` (`ProviderModel` already derives
`PartialEq, Eq`, so the record must too). Every field is `#[serde(default)]`; unknown JSON fields
are ignored so a newer build's extra fields never break an older reader.

### ReasoningControl

```text
Unavailable                       // determined: no reasoning control
ModelManaged                      // determined: model reasons, user cannot choose
Presets { options: Vec<ReasoningOption> }   // determined: user may pick one
```

`Presets.options` is non-empty by construction (an empty list maps to `Unavailable`/`ModelManaged`
at the adapter boundary), and `ModelCapabilities::normalized()` maps a stored or received empty list to `Unavailable`. Order is display order and is provider-defined.

### ReasoningOption

`{ id: String, label: String }`. `id` is the provider-native wire value (for example Anthropic's
`low` or another provider's `minimal`, `balanced`, or numeric-budget identifier) and is what is
validated, persisted and sent. `label` is a display fallback.

### ThinkingStyle

`Adaptive | Manual`. Read only by the Anthropic serializer.

### AttachmentKind (existing, extended)

`Image | Document | Text` in `adapters/types.rs`; gains `Eq, Serialize, Deserialize` with
`#[serde(rename_all = "snake_case")]`. No new enum.

## Stored form

`models.capabilities_json TEXT NULL` holds one serialized `ModelCapabilities`.

| State                                         | Column  | Read result                                        |
| --------------------------------------------- | ------- | -------------------------------------------------- |
| Never determined (legacy provider row, Codex) | `NULL`  | `capabilities: None`                               |
| Determined                                    | JSON    | `Some(record)` (fields may individually be `None`) |
| Unparseable JSON                              | garbage | `None` + `log::warn!` with model id (FR-021)       |

Example (`claude-opus-5`, abbreviated): see `contracts/capabilities-json.md`.

### Transitions

```text
NULL ──provider refresh──▶ record ──provider refresh──▶ record'   (replaced entirely)
NULL ──backfill (local rows only, on list_installed_models)──▶ record
record ──failed refresh──▶ record                                   (unchanged)
```

## ModelRow / ProviderModel / ChatRequest

- `ModelRow.capabilities: Option<ModelCapabilities>` (`storage/models.rs`; `SELECT_COLUMNS`,
  `upsert_model`, `row_to_model` extended).
- `ProviderModel.capabilities: ModelCapabilities` (adapter output; `ModelCapabilities::default()`
  is "everything not determined"). `compose_model_row` copies it into `ModelRow`.
- `ChatRequest`: `effort_level: Option<EffortLevel>` is replaced by
  `reasoning_option: Option<String>` (already validated) and gains
  `capabilities: Option<ModelCapabilities>`. `SendMessageArgs.effort_level` →
  `reasoning_option: Option<String>`.

## Effort Preference (frontend + preferences table, no schema change)

| Attribute | Value                                                                     |
| --------- | ------------------------------------------------------------------------- |
| Scope     | device: `{ kind: 'device', uuid: vaultDeviceUuid }`                       |
| Key       | `chat.reasoning_option.<provider_id>:<remote_id>` (or the plain local id) |
| Value     | the option `id`                                                           |
| Absent    | Auto                                                                      |

Store state added to `useModelsStore` (via `useReasoningPreference`):

| Name                            | Type                                                              | Notes                                                                                                                        |
| ------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `displayModelCapabilities`      | `ComputedRef<ModelCapabilities \| null>`                          | from `installedModels`/`providerModels`; no extra IPC                                                                        |
| `effortLevel`                   | `Ref<string \| null>`                                             | effective option id; `null` = Auto                                                                                           |
| `updateEffortLevel(id \| null)` | function                                                          | validates against `Presets`, optimistic, rollback                                                                            |
| `effortState`                   | `ComputedRef<'selectable' \| 'hidden' \| 'managed' \| 'unknown'>` | drives the popover (R10); `hidden` when no model row is resolved, `unknown` only when a row exists with `capabilities: null` |

### Effort selection state machine

```text
model changes ──▶ token++ ──▶ read pref(model) ──▶ (token & displayModelId unchanged?)
                                   │                        │ no → drop result
                                   ▼ yes
                     saved id ∈ Presets.options ? apply : (clear key; effortLevel = null)
capabilities refresh ──▶ effortLevel ∈ Presets.options ? keep : (clear key; null)
user picks X ──▶ effortLevel = X (optimistic) ──▶ set pref ──fail──▶ rollback + lastError
user picks Auto ──▶ effortLevel = null ──▶ clear pref ──fail──▶ rollback + lastError
```

## TypeScript mirrors (hand-written, repo convention — `src/types/bindings` covers only instance types)

In `src/composables/useModels.ts`: `ModelCapabilities`, `ReasoningControl` (discriminated on `kind`),
`ReasoningOption`. `InstalledModel` and `ProviderModel` (in `useProviders.ts`, type-imported) gain
`capabilities: ModelCapabilities | null`.
