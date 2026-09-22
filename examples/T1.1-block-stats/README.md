# T1.1 — Block Stats (Ethereum)

**Skill exercised:** `substreams-dev`
**Model:** claude-sonnet-4-6
**Result:** Build OK · Run OK · Correctness 100% · 2 trials, both 6/6

## Goal

Per-block stats on Ethereum mainnet: block number, transaction count, total gas used, base fee per gas. Single-map module, minimal scope.

## Prompt

> I want to build a Substreams on Ethereum mainnet that emits per-block statistics.
>
> For each block, output:
> - block number
> - transaction count
> - total gas used
> - base fee per gas
>
> Get it to the point where I can run `substreams build` and `substreams run` against blocks 18000000 through 18000099 and see JSONL output on stdout.
>
> Use whichever Substreams version you think is best. Keep it minimal — single map module.

## What the skill provided

- Manifest skeleton (`specVersion`, `network: mainnet`, `binaries`, single map module)
- `sf.ethereum.type.v2.Block` source type + access patterns (`block.number`, `block.transactions`)
- `buffa` / `buffa-types` dependency callout (generated code names `::buffa` at the crate root, so the `substreams` re-export is not enough)

## Files

- [`substreams.yaml`](substreams.yaml)
- [`Cargo.toml`](Cargo.toml)
- [`proto/stats.proto`](proto/stats.proto)
- [`src/lib.rs`](src/lib.rs)

## Reproduce

```bash
substreams build
substreams run ./substreams.yaml map_block_stats -s 18000000 -t +100 -o jsonl
```
