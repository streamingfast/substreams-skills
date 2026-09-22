# T5.1 — Solana Slot Stats

**Skill exercised:** `substreams-dev` (Solana section)
**Model:** claude-sonnet-4-6
**Result:** Build OK · Run OK · Correctness 100% · 2 trials, both 6/6

## Goal

Per-slot Solana stats: slot number, parent slot, total transactions (incl. failed), successful tx count, failed tx count, total compute units consumed.

## Prompt

> Build a Substreams on Solana mainnet that outputs basic stats for each slot.
>
> For each slot, emit:
> - slot number
> - parent slot
> - total number of transactions (including failed)
> - number of successful transactions
> - number of failed transactions
> - total compute units consumed across all successful transactions

## What the skill provided

- Solana Cargo.toml (`substreams = "0.8.0-beta"`, `substreams-solana = "0.16.0-beta.1"`, `buffa = "0.9"`) — declare `buffa` directly, generated code names `::buffa` at the crate root
- Manifest with `network: solana` + `source: sf.solana.type.v1.Block`
- **`block.transactions` (all) vs `block.transactions()` (successful only)** — confusable; skill calls this out explicitly
- Compute units field path on `meta`

## Files

- [`substreams.yaml`](substreams.yaml)
- [`Cargo.toml`](Cargo.toml)
- [`src/lib.rs`](src/lib.rs)

## Reproduce

```bash
substreams build
substreams run ./substreams.yaml map_block_stats -s 320000000 -t +100 -o jsonl
```

## Notes

This task validated the new Solana skill section end-to-end. Before it landed, agents had near-zero coverage on Solana (no `walk_instructions`, no `b58!` macro, no SPL parsing, no Anchor discriminator math).
