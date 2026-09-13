# Qwen3 is the local model family, with platform-specific presets

Status: accepted
Date: 2026-09-13

## Decision

Holzi uses Qwen3 as its default local model family:

| Target | Preset | Quantization | Purpose |
| --- | --- | --- | --- |
| Desktop | Qwen3-4B | Q4_K_M | Default local assistant and tool-loop model |
| Smartphone | Qwen3-1.7B | Q4_K_M | Default mobile assistant within a smaller memory budget |
| Smartphone, low memory | Qwen3-0.6B | Q4_K_M | Fallback for constrained devices and smoke tests |

The desktop catalog contains all three Qwen3 GGUF profiles so hardware-fit
recommendations can select a smaller model when the host cannot accommodate
Qwen3-4B. Mobile will use the same model family through the planned native
MLC backend; it does not imply that the desktop GGUF loader is used on iOS or
Android.

## Rationale

Qwen3 gives Holzi one model family across platforms while allowing the model
size to follow the device budget. The 0.6B profile is useful for validating
the tool boundary, but is not the preferred general-purpose mobile assistant.
The 1.7B profile is the mobile default because it leaves more capacity for
following multi-step instructions while remaining substantially smaller than
the desktop 4B profile.

Tool calls must remain structured adapter output. Plain text that merely looks
like a shell command is never executable; the existing permission gate remains
mandatory for the host CLI tool.

## Consequences

- The built-in desktop catalog contains Qwen3-0.6B, Qwen3-1.7B, and Qwen3-4B
  Q4_K_M entries.
- The mobile implementation needs a native MLC model mapping for the same
  three logical profiles before mobile inference is enabled.
- Real-model acceptance tests must use a Qwen3-compatible tokenizer and allow
  enough output tokens for a complete structured tool call.
- Users can still import arbitrary GGUFs or provide other HuggingFace models;
  these presets are recommendations, not a hard allow-list.
