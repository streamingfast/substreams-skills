# T1.2 — USDC Transfers (Ethereum)

**Skill exercised:** `substreams-dev`
**Model:** claude-sonnet-4-6
**Result:** Build OK · Run OK · Correctness 100% (94/94 transfers match golden) · 3 trials, all 6/6

## Goal

Index every USDC transfer on Ethereum mainnet. Emit `from`, `to`, `amount`, `tx_hash`, `log_index`, `block_number` per transfer.

## Prompt

> Track all USDC transfers on Ethereum mainnet. Emit from/to/amount per transfer.

## What the skill provided

- ERC20 `Transfer(address,address,uint256)` topic0 hash
- Topic-filter pattern (filter by contract address + topic0)
- `block.transactions()` iteration + `logs_with_calls()` access
- Manifest skeleton with `network: mainnet` + `sf.ethereum.type.v2.Block` source

## Files

- [`substreams.yaml`](substreams.yaml) — manifest
- [`Cargo.toml`](Cargo.toml) — deps (`substreams = "0.8.0-beta"`, `substreams-ethereum = "0.12.0-beta.1"`, `buffa = "0.9"`)
- [`proto/usdc_transfers.proto`](proto/usdc_transfers.proto) — output schema
- [`src/lib.rs`](src/lib.rs) — single map module, ~100 lines

## Reproduce

```bash
substreams build
substreams run ./substreams.yaml map_usdc_transfers -s 18000000 -t +10 -o jsonl
```

## Notes

Golden uses a hand-rolled `decode_uint256` for the amount field (USDC fits in u128 but the function handles full 32-byte values). For any non-trivial token math, prefer `substreams::scalar::BigInt` from the skill's bigint section.
