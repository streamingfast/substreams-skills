# T2.2 — Uniswap V2 Swaps + Token Metadata (Ethereum)

**Skill exercised:** `substreams-dev` (+ `substreams-ethereum` patterns)
**Model:** claude-sonnet-4-6
**Result:** Build OK · Run OK · Correctness 96% · 2 trials, both 6/6
**Note:** Token metadata RPC uses `RpcBatch` (batched `token0`/`token1` + `symbol`/`decimals`).

## Goal

Track every Uniswap V2 swap. For each swap, include pool address, token0/token1 addresses + symbols + decimals, human-readable in/out amounts, sender, recipient, tx hash, log index, block number. **Cache token metadata** in a store — don't re-fetch every block.

## Prompt

> Build a Substreams on Ethereum mainnet that tracks Uniswap V2 swaps.
>
> For each swap emit: pool, token0/1 address+symbol+decimals, amounts in/out (human-readable), sender, recipient, tx hash, log index, block number.
>
> Token metadata (symbol, decimals) should be cached — don't re-fetch for the same token across blocks. Amounts must be human-readable (divided by `10^decimals`), not raw wei.

## What the skill provided

- ABI generation via `build.rs` + `substreams-ethereum-abigen` (writes `src/abi/*.rs`)
- `RpcBatch::execute()` pattern for calling `symbol()` / `decimals()` from a map handler
- Store-cache pattern (`set_if_not_exists`) so token metadata isn't re-fetched per block — explicit anti-pattern callout warning against per-block `HashMap` cache
- `BigInt` decimal scaling for human-readable amounts

## Files

- [`substreams.yaml`](substreams.yaml) — multi-module pipeline (map → store → map)
- [`Cargo.toml`](Cargo.toml)
- [`build.rs`](build.rs) — abigen at build time
- [`abi/`](abi/) — UniswapV2Pair + ERC20 ABI JSON
- [`src/lib.rs`](src/lib.rs)
- [`src/abi/`](src/abi/) — generated bindings (committed, regenerable via `cargo build`)
- [`proto/uniswap_v2.proto`](proto/uniswap_v2.proto)

## Reproduce

```bash
substreams build
substreams run ./substreams.yaml map_swaps -s 18000000 -t +100 -o jsonl
```

## Caveat — MKR `bytes32 symbol()`

The 4% correctness gap is one token: **MKR**. MKR predates the modern ERC20 standard and returns `bytes32` from `symbol()`, not `string`. Both the golden and the agent emit empty/wrong `symbol` for swaps involving MKR. The pipeline still emits the swap and decodes amounts correctly — only the `symbol` field is broken on this one token.

Real-world fix: try the ABI-standard `string` decoding first, fall back to `bytes32` decoding on failure. Out of scope for this example.
