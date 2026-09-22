# T3.2 — Cross-DEX Volume Aggregation, graph_out (Ethereum)

**Skill exercised:** `substreams-dev`
**Model:** claude-sonnet-4-6
**Result:** Build OK · Run OK · Correctness 100% · 2 of 3 latest trials at 6/6 (one 4/6 from `HashMap` aggregation breaking CREATE/UPDATE semantics)

## Goal

Aggregate swap volume across Uniswap V2 + V3 per pool per UTC day. Output a `graph_out` module emitting `EntityChanges` (subgraph entities). Token metadata cached in a store. Entity ID format and field names fixed in the prompt for downstream subgraph compatibility.

## Prompt

Multi-paragraph prompt with a strict entity schema:
- Entity type `PoolDayVolume`, ID `"{protocol}:{pool}:{dayBucket}"`, fields `id` / `pool` / `protocol` / `dayBucket` / `volumeToken1`
- First write per (protocol, pool, day) = `OPERATION_CREATE` with all 5 fields
- Subsequent writes = `OPERATION_UPDATE` with only `volumeToken1`
- `initialBlock: 18000000` (test range, not protocol genesis)


## What the skill provided

- **Embedded `EntityChanges` proto** — agents previously hallucinated `sf.substreams.sink.subgraph.v1.DatabaseChanges` (mixing graph-out package with SQL sink message). Skill now embeds the literal `.proto` content for `sf.substreams.sink.entity.v1.EntityChanges` so agents copy verbatim.
- **`initialBlock` semantics** — `initialBlock = max(--start-block, manifest.initialBlock)`. Pin to test range, not protocol genesis (V3 factory at 12369621 → 5.6M-block backfill).
- **`substreams-entity-change` crate workaround** — crate is locked to an old `substreams` and still builds on `prost`. Skill embeds the proto inline rather than importing the broken crate.
- Multi-source module pattern (V2 + V3 swap events feeding the same store).

## Files

- [`substreams.yaml`](substreams.yaml) — multi-source pipeline + `graph_out` module
- [`Cargo.toml`](Cargo.toml)
- [`build.rs`](build.rs)
- [`abi/`](abi/), [`src/abi/`](src/abi/)
- [`proto/dex_volume.proto`](proto/dex_volume.proto), [`proto/entity.proto`](proto/entity.proto) — embedded `EntityChanges` workaround
- [`src/lib.rs`](src/lib.rs)

## Reproduce

```bash
substreams build
substreams run ./substreams.yaml graph_out -s 18000000 -t +100 -o jsonl
```

## Notes

The `EntityChanges` proto is embedded in this example because the upstream `substreams-entity-change` crate is unmaintained on the modern toolchain. Once a release lands on buffa, the embedded proto can be removed.
