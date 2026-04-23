# Timeline Export (Compact Narrative Artifact)

`--timeline-output` writes a lightweight JSON artifact that summarizes an execution run without
exporting a full instruction/event trace.

This is intended for sharing a concise “execution story” (what ran, what changed, what warned),
especially when a full `--trace-output` would be unnecessarily large.

## CLI

Export a compact narrative to a JSON file:

```bash
soroban-debug run \
  --contract contract.wasm \
  --function main \
  --args '["alice", "bob", 100]' \
  --timeline-output timeline.json
```

You can also export both artifacts:

```bash
soroban-debug run \
  --contract contract.wasm \
  --function main \
  --args '["alice", "bob", 100]' \
  --trace-output trace.json \
  --timeline-output timeline.json
```

## What’s inside

The artifact is JSON with a small, versioned schema:

- `schema_version`: Format version for forwards-compatible parsing.
- `created_at`: RFC3339 UTC timestamp of when the file was produced.
- `run`: Basic run metadata (contract path, function, args JSON, result/error, budget, events count).
- `pauses`: Best-effort pause points (currently includes entry breakpoint hits in `run`).
- `stack_summary`: Best-effort end-of-run call stack summary.
- `deltas.storage`: Storage diff summary (added/modified/deleted keys, alerts, and truncation marker).
- `warnings`: Warnings emitted during narrative construction (e.g. triggered storage alerts).

## Notes

- This export is intentionally **not** a raw trace format and is expected to stay compact.
- Large storage diffs are size-capped and set `deltas.storage.truncated=true`.
