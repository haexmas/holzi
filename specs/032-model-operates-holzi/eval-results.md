# Evaluation results (T046)

## Qwen3-4B

Measured on 2026-10-01 with Q4_K_M, CPU, a release build, deterministic settings,
and no system prompt, using the same setup as the application.

- 16 of 30 sentences reached the expected result (53%).
- German and English each reached 8 of 15.
- Smalltalk reached 6 of 6; reads reached 4 of 11; changes reached 6 of 13.
- `reachRate` was 0.54, with no search step (`extraSteps` 0) and no spurious call.
- Two runs produced the same report, satisfying SC-008.
- The model never called `find_actions`; it answered in text or called a wrong core
  tool for the 11 sentences outside the core offer. Two core sentences also ended
  as text.
- The self-test reached 4 of 5 (0.8). Five self-test sentences took about 1.3
  minutes; the full 30-sentence run took about 7.8 minutes.

The catalog therefore records Qwen3-4B as `tool_use: supported`, and
`SELF_TEST_PASS` remains 0.6. This is one measured model, not evidence for all
catalog entries.

## Not measured

Qwen3 0.6B and 1.7B were not installed in the measurement environment. A Claude
model was not measured because no API key was available. The full T046 task
remains open until those measurements and the required self-test comparison are
available.
