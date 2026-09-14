---
id: graphify-context-map
kind: constitution_fragment
atom_source: graphify-context-map
tags: [graphify, context, tokens, retrieval]
---
# Capability: graphify context maps

**Status**: Opt-in through molecule `com.github.haexmas.atoms.graphify-first-authoring`.
**Applies to**: agents working in a project with a usable `graphify-out/graph.json`.

## Purpose

Use a bounded, task-specific context map before reading broad portions of a
repository. A context map is a compact list of the files, symbols, and
relationships most relevant to one intent. It is a navigation aid, not a
replacement for reading the source of a selected artifact.

This capability complements the graphify-first authoring rule. It does not
replace the required graph consultation, and it must not cause a full graph or
full repository dump to be copied into the agent context.

## Context-map operation

When a task crosses files or the relevant implementation is not obvious, the
agent SHOULD perform this operation before broad file reads:

1. State the concrete intent as a question, for example
   `where is the install transaction assembled and what calls it?`.
2. Choose a small token budget appropriate to the task. Start around 800–1500
   tokens and increase it only when the returned relationships are insufficient.
3. Run the plain CLI query with an explicit budget:

   ```text
   graphify query "<intent>" --budget <N>
   ```

   Use `--dfs` for a narrow dependency path and `graphify path` or
   `graphify explain` when the map needs one specific relationship or symbol.
4. Treat the result as a map. Extract the cited source locations, symbols, and
   relationships; then read only the smallest source slices needed to answer
   the task.
5. If the result is truncated or ambiguous, narrow the question or make one
   additional focused query. Do not compensate by dumping the repository.

The resulting map SHOULD preserve, when available:

- the task intent and token budget;
- selected files and symbols;
- the relevant edge or call relationship;
- source locations for every claim; and
- an explicit truncation or failed-consultation note.

For a local one-file edit with an already-known symbol, the operation is not
required. For authoring a new named artifact, the graphify-first authoring rule
still applies even when a context map would be unnecessary.
