# Contract: stored and wire capability record

One serde representation serves both the `models.capabilities_json` column and the camelCase Tauri
payloads (no separate mapping DTO). Structs use `rename_all = "camelCase"`; enums use an internal
`kind` tag with `snake_case` variants; all fields `#[serde(default)]`; unknown fields are ignored.

## Shape

```jsonc
{
  "reasoning": {
    "kind": "presets",
    "options": [{ "id": "low", "label": "low" } /* … */],
  },
  //          | { "kind": "unavailable" } | { "kind": "model_managed" } | null   (null = not determined)
  "acceptedAttachmentKinds": ["text", "image", "document"], // [] = none; null = not determined
  "thinkingStyle": "adaptive", // "adaptive" | "manual" | null
}
```

## Examples

| Case                                         | Record                                                                                                                                                    |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Newer Claude model                           | `{"reasoning":{"kind":"presets","options":[low,medium,high,xhigh,max]},"acceptedAttachmentKinds":["text","image","document"],"thinkingStyle":"adaptive"}` |
| Claude model with effort but manual thinking | `{"reasoning":{"kind":"presets","options":[low,medium,high,max]},…,"thinkingStyle":"manual"}`                                                             |
| Claude model that thinks but has no effort   | `{"reasoning":{"kind":"model_managed"},…,"thinkingStyle":"manual"}`                                                                                       |
| Local Qwen3 (reasoning family)               | `{"reasoning":{"kind":"model_managed"},"acceptedAttachmentKinds":[],"thinkingStyle":null}`                                                                |
| Local model, no reasoning                    | `{"reasoning":{"kind":"unavailable"},"acceptedAttachmentKinds":[],"thinkingStyle":null}`                                                                  |
| Codex delegate model                         | `{"reasoning":null,"acceptedAttachmentKinds":null,"thinkingStyle":null}` (Rust writes `NULL` column for an all-`None` record)                             |

## Anthropic wire → record (used as the wiremock fixture set)

Input is `GET /v1/models` `data[i].capabilities`. See `research.md` R3 for the full table. Required
fixture cases for `anthropic_tests.rs`:

1. Full tree, adaptive + all effort levels → `Presets` (5 options), `Adaptive`, kinds incl. `Document`.
2. Effort without `xhigh` (levels: low, medium, high, max), `enabled` thinking only → `Presets`
   (4 options), `Manual`.
3. `thinking.supported = true`, `effort.supported = false` → `ModelManaged`.
4. `thinking.supported = false`, `effort.supported = false` → `Unavailable`, `thinkingStyle = null`.
5. `capabilities` object absent → all three fields `None`; the listing still succeeds.
6. `image_input` present, `pdf_input` absent → `acceptedAttachmentKinds = None` (FR-002).
7. An unknown top-level capability leaf is ignored, while an unknown provider-native effort key
   with `supported: true` becomes a selectable option and is preserved unchanged.

## Compatibility

Adding a field later needs no migration: the column is opaque JSON and readers default missing
fields to `None` ("not determined"). Removing or renaming a field is a breaking stored-format change
and requires a backfill.
