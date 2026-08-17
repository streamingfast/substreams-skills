---
name: substreams-ethereum
description: >
  Develop Substreams for Ethereum and EVM contracts — ABI codegen (Abigen/build.rs),
  event decoding via match_and_decode, raw topic0 decoding without an ABI, eth_call /
  RpcBatch, and token-metadata stores. Always decode log data into typed structured
  protobuf objects (never raw bytes or JSON); one protobuf message type per event.
  Use when building EVM Substreams, decoding contract events, ERC-20/721/1155,
  Uniswap/DEX pools, or when the user mentions an Ethereum contract address, ABI,
  topic0, Solidity, or an EVM chain (Polygon, Arbitrum, Base, BSC, Optimism).
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.6.0
  author: StreamingFast
  documentation: https://docs.substreams.dev/how-to-guides/develop-your-own-substreams/ethereum
---

# Substreams Ethereum Expert

Expert assistant for **Ethereum / EVM contract** Substreams: log extraction, ABI decoding, and RPC enrichment.
Cross-cutting manifests, stores, and performance live in **`substreams-dev`**. SQL module → **`substreams-sql`**. Self-managed sink ops → **`substreams-sink-deploy-local`**. StreamingFast-hosted SQL sink → **`substreams-hosted-sink`**.

EVM is **not** Solana. Do not use instruction/account/discriminator patterns here. Decoding model = **logs + topics + ABI**.

## Hard rule: decode log data into structured objects

**Always decode log `data` and `topics` into typed fields.** Emitting only presence/address filters, raw bytes, hex blobs, or a JSON string of the payload is **invalid** for contract Substreams in this skill.

### Hard rule: one protobuf message type per event

**Each selected event gets its own dedicated protobuf message** with fields matching that event's parameters.

| Required | Forbidden |
|---|---|
| e.g. `message Swap { … }`, `message Mint { … }`, `message Transfer { … }` | One generic `Event` / `Log` with optional fields for every event |
| Distinct message per event name/topic0 the user asked for | Shared bag: `string event_name` + `map` / sparse optional args |
| Module output may wrap them: `repeated Swap swaps` **and** `repeated Mint mints`, or `oneof` of **those** messages | Single table of mixed rows with unused columns for other events |
| Decode indexed params from `topics[1..]` and non-indexed from `data` into **named typed values** | Leave `data` as `bytes` / hex |
| `uint256` → `string` (decimal) via `BigInt`; addresses → `0x`-prefixed hex `string` | Dump the whole log as JSON |
| | `log_data_json`, `raw_data`, `args_json`, `serde_json::to_string` |

**Why:** sinks, SQL (`schema.table` per message), and consumers need stable per-event schemas — not opaque blobs or a one-size-fits-all row.

```proto
// ✅ One message type per event; wrapper holds several lists (or a oneof)
message Swap {
  uint64 block_number = 1;
  string tx_hash = 2;
  uint32 log_index = 3;
  string pool = 4;
  string amount0_in = 5;   // uint256 → decimal string
}
message Mint { uint64 block_number = 1; string owner = 2; string amount = 3; }
message Events {
  repeated Swap swaps = 1;
  repeated Mint mints = 2;
}

// ❌ Forbidden: one mega-message for all events
message GenericEvent {
  string event = 1;      // "swap" | "mint" | …
  string pool = 2;       // only set for swap
  bytes raw_data = 3;
}
```

**Scope:** applies to every **selected event** you emit. Topic0 matching alone is not enough — params must be decoded. Pure block/tx stats modules that never claim to decode a contract (T1.1) are exempt; any module that filters on a contract/event **must** decode. **`uint256` never fits in `u64`** — use `substreams::scalar::BigInt`, emit a decimal `string`; silent truncation is a correctness bug, not a style issue.

## Pre-flight (ALWAYS run first)

**Do not write Rust, protos, or manifests until the user has answered (or already stated) the items below.**

### How to ask (MANDATORY)

Same protocol as `substreams-dev` (and all Substreams skills):

1. **One question per turn** — never a multi-question dump.
2. **Offer choices** for every question (platform choice UI or numbered options).
3. **Always include** a final **"Other / enter a custom answer"** option.
4. Skip items the user already stated; wait for each answer before the next question.

> Examples T4.1 / T4.2 are **silent ships** — a full pipeline built from a vague prompt ("track whale activity") with nothing asked. Vague scope is a signal to ask, not to guess.

### Step 1 — Scope inputs (ask missing items one at a time)

Walk this list **top to bottom**, one question per turn:

| Order | Question focus | Example choices (always end with custom) |
|---|---|---|
| 1 | **Network** | Ethereum mainnet · Base · Arbitrum · Polygon · **Other / custom** |
| 2 | **Contract address(es)** | Known protocols from [common-contracts](./references/common-contracts.md) · **Other / paste address** |
| 3 | **Event(s)** | Named event presets for that contract · All events · **Other / list events or topic0** |
| 4 | **Fields / output shape** | Suggested field set · Minimal (block, tx, log_index, key params) · **Other / describe fields** |
| 5 | **Enrichment** | None · Token symbol/decimals via RPC · USD pricing · **Other / describe** |
| 6 | **Output / sink destination** | Full list below · **Other / custom** |
| 7 | **Block range** | Suggested window · Recent head · **Other / enter blocks** |

**"All events" + SQL/ClickHouse:** one protobuf message **and** typically one table per event. Prefer a **ladder** for a first hosted deploy: high-value events (e.g. `Swap`) → liquidity (`Mint`/`Burn`) → full surface. Before ClickHouse, rename **reserved column names** (`index`, `keys`, …) per `substreams-sql`.

#### Output destination — its own turn

Ask only when destination is unknown. Offer **all** of these; last = custom:

| Choice | Follow-on |
|---|---|
| **`substreams run` only** | Verify with CLI/GUI first |
| **SQL — self-managed** | `substreams-sql` → `substreams-sink-deploy-local` |
| **SQL — StreamingFast hosted sink** | `substreams-sql` → **`substreams-hosted-sink`** |
| **The Graph / graph-out** | `substreams-sink` (EntityChanges) |
| **Custom app sink** | `substreams-sink` |
| **Not sure yet** | Start with run-only |
| **Other / enter a custom answer** | Free-text |

If the user says only "SQL", ask **self-managed vs hosted** in a **separate** turn (plus custom). Hosted still needs a correct SQL module first.

If the user names a protocol without an address, propose the mainnet address from [references/common-contracts.md](./references/common-contracts.md) as a **choice**, then still confirm events in a later turn.

### Step 2 — Decoding source decision (ABI vs Solidity vs known signature)

**One turn when ambiguous.** Prefer the highest-quality source available.

| Choice | When to use | How you decode |
|---|---|---|
| **ABI JSON** | User has ABI / verified on Etherscan / published | `Abigen` in `build.rs` → `match_and_decode` (**preferred**) |
| **Solidity source** | No ABI JSON; source available | Derive canonical signature → `keccak256` → topic0; hand-decode (T6.1) |
| **Known signature** | ERC-20/721/1155 or documented event | Known topic0 constant + `ethabi` decode |
| **Other / enter a custom answer** | User describes another source | Follow their layout; still structured decode |

**Decision rules:**

1. If the user provides or can obtain an **ABI JSON** → Abigen path. This is the default and the least error-prone.
2. Else if **Solidity source** → derive topic0 from the canonical signature.
3. Else if **standard token/interface** → known topic0 from [common-contracts](./references/common-contracts.md).
4. Else → ask this question (choices above). Do **not** invent topic0 hashes or param orders.

Deep dive: [references/abi-codegen.md](./references/abi-codegen.md).

### Step 3 — Filter strategy

From contract address(es) + event(s):

1. **In-module filter** — check `log.address` and `topics[0]` before decoding (cheapest reject).
2. **Index module** (recommended when scanning large ranges) — emit keys like `contract:<hex>`, `event:<name>` so consumers skip empty blocks.
3. **Foundational packages** — `substreams init` → **ethereum-common** for cached address/topic filters.

Details: [references/abi-codegen.md](./references/abi-codegen.md).

---

## Cargo.toml

```toml
[dependencies]
substreams = "0.7"
substreams-ethereum = "0.11"
prost = "0.13"
prost-types = "0.13"
hex = "0.4"
hex-literal = "0.4"          # hyphen in Cargo.toml, underscore in `use hex_literal::hex`
ethabi = "17"                # REQUIRED on the Abigen path (see below) — must be 17, not 18

[build-dependencies]
substreams-ethereum = "0.11" # REQUIRED for Abigen in build.rs
prost-build = "0.13"

[lib]
crate-type = ["cdylib"]

[profile.release]
lto = true
opt-level = "s"
strip = "debuginfo"
```

`substreams-ethereum` must be in **both** `[dependencies]` and `[build-dependencies]` when using `Abigen`.

**`ethabi` is required on the Abigen path — and must be `17`, not `18`.** Abigen writes bare `ethabi::ParamType` / `ethabi::Token` paths into the file it generates in **your** crate, and `substreams-ethereum` does **not** re-export `ethabi`, so it must be a direct dependency of yours or the generated module fails to resolve. (All three Abigen examples in this repo declare it; the hand-decoding ones — T1.2, T2.1, T6.1 — correctly do not.)

Pin **`17`**: `substreams-ethereum-core` pins `ethabi 17`, so `"18"` still compiles but silently links a **second** copy of the whole stack (`ethabi`, `ethereum-types`, `primitive-types`, …) into the wasm binary, and its types are not interchangeable with the crate's. Bigger binary, no benefit.

**`getrandom` and `init!()` are not required on 0.11.** The crate's own docs still say to add a `getrandom` dependency and call `substreams_ethereum::init!();` — that guidance is stale. `substreams-ethereum` already declares `getrandom = { features = ["custom"] }` for `wasm32-unknown-unknown` itself, and cargo unifies the feature. No example in this repo does either, and all build. Both are harmless if present, but `init!()` only registers a handler that *always returns an error* — it satisfies the linker, it does not provide randomness.

## build.rs (ABI codegen)

```rust
fn main() {
    substreams_ethereum::Abigen::new("UniswapV3Pool", "abi/uniswap_v3_pool.json")
        .expect("Failed to load ABI")
        .generate()
        .expect("Failed to generate bindings")
        .write_to_file("src/abi/uniswap_v3_pool.rs")
        .expect("Failed to write bindings");

    prost_build::compile_protos(&["proto/uniswap_v3.proto"], &["proto/"]).unwrap();
}
```

Create `src/abi/mod.rs` declaring each generated module (`pub mod uniswap_v3_pool;`), and `mod abi;` in `lib.rs`.

## Manifest

```yaml
specVersion: v0.1.0
package:
  name: my_eth_substreams
  version: 1.6.0
  url: https://github.com/myorg/my-eth-substreams   # set both — silences build warnings
  description: Decoded Uniswap V2 swaps on Ethereum mainnet
network: mainnet

protobuf:                    # REQUIRED — `substreams pack` fails without it
  files:
    - my_events.proto
  importPaths:
    - ./proto

binaries:                    # REQUIRED — `substreams pack` fails without it
  default:
    type: wasm/rust-v1
    # filename = Cargo.toml [package] name with hyphens → underscores,
    # which is NOT necessarily the package.name above
    file: ./target/wasm32-unknown-unknown/release/my_eth_substreams.wasm

modules:
  - name: map_events
    kind: map
    initialBlock: 18000000   # pin to test/data range — not genesis
    inputs:
      - source: sf.ethereum.type.v2.Block
    output:
      type: proto:mypackage.v1.MyOutput
```

`protobuf:` and `binaries:` are **not optional** — every working example in this repo carries both, and a manifest without them does not pack.

`sf.ethereum.type.v2.Block` is a **well-known source** — no `imports:` entry is needed for the block type. (If you do import the eth spkg, it is `substreams-ethereum`, never `sf-ethereum`, which does not exist.)

## Canonical loop: transactions + logs

```rust
use substreams::errors::Error;
use substreams::Hex;
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::Event;   // REQUIRED for .match_and_decode()

// Hex::encode is lowercase and has NO 0x — always re-prefix for emitted addresses/tx hashes.
fn hex0x(b: &[u8]) -> String { format!("0x{}", Hex::encode(b)) }

const POOL: [u8; 20] = hex_literal::hex!("b4e16d0168e52d35cacd2c6185b44281ec28c9dc");

#[substreams::handlers::map]
fn map_events(block: eth::Block) -> Result<MyEvents, Error> {
    let mut out = Vec::new();

    for trx in block.transactions() {              // successful txs only
        let tx_hash = hex0x(&trx.hash);

        for (log, _call) in trx.logs_with_calls() {
            if log.address != POOL {               // 1) cheapest reject: address
                continue;
            }
            // 2) MUST decode into typed fields (not JSON, not raw bytes)
            // Module path = write_to_file stem (e.g. uniswap_v2_pair), not Abigen::new name.
            if let Some(swap) = abi::uniswap_v2_pair::events::Swap::match_and_decode(log) {
                out.push(Swap {
                    block_number: block.number,
                    tx_hash: tx_hash.clone(),
                    log_index: log.index,
                    sender: hex0x(&swap.sender),
                    amount0_in: swap.amount0_in.to_string(),  // BigInt → decimal string
                });
            }
        }
    }

    Ok(MyEvents { swaps: out })
}
```

For a handler processing **one** event type, `block.events::<E>(&[&ADDR])` collapses the whole nest into a single loop — see [references/abi-codegen.md](./references/abi-codegen.md).

### Critical rules

| Rule | Why |
|---|---|
| **`use substreams_ethereum::Event;`** | Without this trait import, `.match_and_decode()` / `.decode()` do not resolve. Error reads *"no method named `decode` found"* — the single most common EVM build failure. |
| **`block.transactions()` = successful only** | Filters `status == 1`. Good for protocol events. |
| **`trx.logs_with_calls()` excludes reverted calls** | Skips `state_reverted` calls and sorts by ordinal. A reverted internal call still emits logs that never reached chain state. |
| **Never iterate `call.logs` directly** | You would include logs from reverted sub-calls. Use `logs_with_calls()` or `block.logs()`. |
| **Need failed txs / full counts?** | Iterate `&block.transaction_traces` (all traces) and check `status` yourself. |
| **Filter `log.address` then `topics[0]` first** | Cheapest reject before decoding. |
| **`uint256` → `BigInt` → decimal string** | `u64`/`u128` silently truncate real token amounts. |
| **Emitted addresses/tx = `0x` + lowercase hex** | `Hex::encode` has **no** `0x`. Use `format!("0x{}", Hex::encode(…))` (or `hex0x` above). Keep **store keys** the same format you use when looking them up. |
| **Do not invent topic0 or param order** | Derive from ABI or `keccak256` of the canonical signature. |
| **One proto message per event** | `Swap`, `Mint`, … — never a single generic event for all types. |

## Event matching and decoding

Matching topic0 is only step 1. **You must continue and decode the payload** into a structured output object (see [Hard rule](#hard-rule-decode-log-data-into-structured-objects)).

### With an ABI (preferred)

```rust
if let Some(swap) = abi::uniswap_v2_pair::events::Swap::match_and_decode(log) {
    // swap.amount0_in is a substreams::scalar::BigInt — already typed
}
```

`match_and_decode` checks topic0 **and** decodes in one step; it returns `None` on mismatch, so an explicit topic0 guard is optional (though still the cheaper reject when scanning many logs).

### Without an ABI — raw topic0

topic0 = `keccak256("EventName(type1,type2,...)")` over the **canonical signature**: no parameter names, no spaces, `uint256` not `uint`.

```rust
// keccak256("Swap(address,uint256,uint256,uint256,uint256,address)")
const SWAP_TOPIC: [u8; 32] =
    hex_literal::hex!("d78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822");

if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
    continue;
}
```

**Indexed vs non-indexed is the thing agents get wrong:** `topics[0]` = event signature; `topics[1..]` = **indexed** params, one 32-byte word each, in declaration order (max 3); `data` = **non-indexed** params, ABI-encoded 32-byte words. An `address` in a topic is left-padded — it is `topic[12..32]`. Use `log.topics.len()` to disambiguate overloaded events.

Verified topic0 constants: [references/common-contracts.md](./references/common-contracts.md). Full decoding guide: [references/abi-codegen.md](./references/abi-codegen.md).

## Enrichment: eth_call / RpcBatch

Contract state that is not in the log (a pool's `token0`, a token's `symbol`/`decimals`) requires an RPC call.

```rust
use substreams_ethereum::rpc::RpcBatch;

// execute() → Result<_, String> (not a std Error — do not `?` into substreams::Error).
// Per-call failure is more common: decode → Option (None on revert / bad return).
let batch = match RpcBatch::new()
    .add(abi::erc20::functions::Symbol {}, addr.to_vec())
    .add(abi::erc20::functions::Decimals {}, addr.to_vec())
    .execute()
{
    Ok(b) => b,
    Err(_) => return Ok(Default::default()),
};

let symbol = RpcBatch::decode::<_, abi::erc20::functions::Symbol>(&batch.responses[0])
    .unwrap_or_else(|| "UNKNOWN".to_string());
let decimals = RpcBatch::decode::<_, abi::erc20::functions::Decimals>(&batch.responses[1])
    .map(|d| d.to_u64() as u32)
    .unwrap_or(18u32);
```

**RPC is the top performance and correctness trap on EVM.** Rules:

1. **Batch** related calls into one `RpcBatch` — never one `execute()` / `.call()` per field.
2. **Cache in a `set_if_not_exists` store**, keyed by address — pay the RPC once per contract, not once per block. **Never a `HashMap` inside a map handler**: it is rebuilt every block and re-issues every call.
3. **Always handle failure.** Non-compliant tokens exist (`symbol()` returning `bytes32`, or missing entirely). Decode returns `None` — fall back to a default, do not `unwrap()`.
4. Prefer log-derived data when the event already carries the field.
5. Prefer **map (fetch) → store (cache) → map (consume)** so RPC stays out of the store handler (T3.1).

Eval note: T3.1 agents that skipped `pool.token0()`/`token1()` resolution averaged **41%** correctness; with the `RpcBatch` + cache-store pattern, **100%**. This is the highest-value pattern in this skill.

Full worked example (`map_pool_tokens` → `store_pool_tokens` → `map_swaps`): [references/rpc-and-tokens.md](./references/rpc-and-tokens.md).

## Project bootstrap options

| Goal | Approach |
|---|---|
| Contract with a published ABI | `substreams init` → **ethereum-minimal**, drop ABI in `abi/`, wire `Abigen` |
| Full block, custom decode | Hand-written map on `sf.ethereum.type.v2.Block` (examples T1.2 / T6.1) |
| Filter by address/topic | `substreams init` → **ethereum-common** (cached filters) |
| Events → SQL | This skill for the map, then `substreams-sql` for `db_out` |

After generators, still enforce the pre-flight event list and address filters.

## Implementation checklist

1. Pre-flight complete (network, contract, events, fields, enrichment, sink, blocks).
2. Decoding path chosen (ABI / Solidity source / known signature).
3. `network:` set for the target chain, input `sf.ethereum.type.v2.Block`, crate versions as above.
4. `substreams-ethereum` in **both** `[dependencies]` and `[build-dependencies]` if using `Abigen`; `use substreams_ethereum::Event;`. Store handlers also need `StoreNew` + the accessor trait in scope.
5. Loop: `block.transactions()` + `logs_with_calls()` + address filter + topic0 filter; only requested events matched.
6. **Decode** into **typed fields**, **one protobuf message type per event**; `uint256` → `BigInt` → decimal `string`; addresses/tx → **`0x` + lowercase hex** (not bare `Hex::encode`).
7. Any RPC batched **and** cached in a `set_if_not_exists` store.
8. `initialBlock` = start of needed range; test with `substreams run -s <block> -t +100 -o jsonl`.
9. Self-check: no generic mega-message; no `*_json` / `raw_data` blobs; no `u64` token amounts.
10. If sink is **ClickHouse / from-proto**: apply reserved-name renames and PK/ORDER BY rules from `substreams-sql` before first deploy.

## Examples in this repo

| Example | Pattern |
|---|---|
| T1.1 block-stats | Block-level stats over `block.transaction_traces` (no decoding) |
| T1.2 usdc-transfers | ERC-20 `Transfer` topic0 filter + `uint256` decode |
| T2.1 nft-mints | ERC-721 `Transfer` with zero-address `from` (mint detection) |
| T2.2 univ2-swaps | `Abigen` + `match_and_decode` + store-backed token metadata |
| T2.3 sql-sink | EVM events → Postgres `db_out` |
| T3.1 univ3-usd-price | **Canonical RPC pattern:** `RpcBatch` + `set_if_not_exists` + sqrtPriceX96 pricing |
| T3.2 cross-dex-volume | Multi-contract aggregation → `graph_out` |
| T6.1 eth-univ2-no-abi | topic0 derived from Solidity source, no ABI JSON |
| T4.1 / T4.2 | **Cautionary** — silent ship, no clarifying questions asked |

## Troubleshooting

| Symptom | Fix |
|---|---|
| `no method named 'decode' found` / `match_and_decode` unresolved | Add `use substreams_ethereum::Event;` |
| `Abigen` not found in `build.rs` | Add `substreams-ethereum` to `[build-dependencies]` |
| Empty output | Wrong address (when comparing **as strings**, both sides lowercase and same `0x` policy), wrong topic0, or `initialBlock` past the data |
| Fewer events than expected | Iterating only top-level `receipt.logs` of a subset, or filtering failed txs incorrectly |
| Extra events vs Etherscan | Iterating `call.logs` directly and including `state_reverted` calls — use `logs_with_calls()` |
| Token amounts wrong / negative / truncated | `uint256` parsed into `u64`/`i64` — use `BigInt`, emit decimal `string` |
| `symbol()` returns garbage or panics | Non-compliant token (`bytes32` symbol) — handle `None`, don't `unwrap()` |
| Very slow / RPC timeouts | Unbatched or uncached `eth_call` — batch + `set_if_not_exists` store |
| spkg import 404 | Use `substreams-ethereum`, NOT `sf-ethereum` (doesn't exist); verify the release exists |
| `hex_literal` unresolved | Cargo key is `hex-literal` (hyphen); `use hex_literal::hex` (underscore) |
| `getrandom` "not supported by default" on wasm32 | You pinned a stray `getrandom` **without** `features = ["custom"]`, or an old `substreams-ethereum`. On 0.11 the crate supplies this itself — remove your own `getrandom` dep rather than adding one |
| `failed to resolve: use of undeclared crate 'ethabi'` in `src/abi/*.rs` | Abigen's generated code needs `ethabi` as **your** direct dependency — add `ethabi = "17"` |
| Duplicate `ethabi` / `ethereum-types` in the build | You pinned `ethabi = "18"`; core pins `17`. Use `"17"` |
| `cannot find function 'new' in ... Store` / `no method 'set_if_not_exists'` | Store handler missing `use substreams::store::StoreNew;` (the macro emits `::new()`) and the accessor trait |
| ClickHouse `SYNTAX_ERROR` on `index` / `keys` | Rename columns in proto (`log_index`, `topic_keys`) — `substreams-sql` |

## Resources

* [ABI codegen & decoding (ABI vs Solidity vs raw)](./references/abi-codegen.md)
* [eth_call, RpcBatch, token metadata stores](./references/rpc-and-tokens.md)
* [Common contracts, topic0 hashes & standards](./references/common-contracts.md)
* [Ethereum docs](https://docs.substreams.dev/how-to-guides/develop-your-own-substreams/ethereum)
* [substreams-ethereum crate](https://github.com/streamingfast/substreams-ethereum)
* Cross-cutting: **`substreams-dev`** · SQL module: **`substreams-sql`** · Hosted sink: **`substreams-hosted-sink`** · Self-managed sink: **`substreams-sink-deploy-local`** · Testing: **`substreams-testing`**
