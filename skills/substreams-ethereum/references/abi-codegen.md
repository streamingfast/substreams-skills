# ABI Codegen & Event Decoding

Deep dive for the `substreams-ethereum` skill. Three decoding paths, in order of preference: **ABI JSON → Solidity source → known signature**.

## Path 1 — ABI JSON with Abigen (preferred)

### Layout

```
my-substreams/
├── abi/
│   ├── erc20.json
│   └── uniswap_v3_pool.json
├── build.rs
├── proto/
└── src/
    ├── abi/
    │   ├── mod.rs            # hand-written: pub mod erc20; pub mod uniswap_v3_pool;
    │   ├── erc20.rs          # generated — do not edit
    │   └── uniswap_v3_pool.rs
    └── lib.rs
```

### build.rs

```rust
fn main() {
    substreams_ethereum::Abigen::new("Erc20", "abi/erc20.json")
        .expect("Failed to load ERC20 ABI")
        .generate()
        .expect("Failed to generate ERC20 bindings")
        .write_to_file("src/abi/erc20.rs")
        .expect("Failed to write ERC20 bindings");

    substreams_ethereum::Abigen::new("UniswapV3Pool", "abi/uniswap_v3_pool.json")
        .expect("Failed to load pool ABI")
        .generate()
        .expect("Failed to generate pool bindings")
        .write_to_file("src/abi/uniswap_v3_pool.rs")
        .expect("Failed to write pool bindings");

    prost_build::compile_protos(&["proto/uniswap_v3.proto"], &["proto/"]).unwrap();
}
```

`substreams-ethereum` must be in **`[build-dependencies]`** for this to compile, and in `[dependencies]` for the runtime trait. Generated files are written into `src/`, so they are compiled as normal modules — declare them in `src/abi/mod.rs` and add `mod abi;` to `lib.rs`.

### What Abigen generates

For each ABI it produces two namespaces:

* `abi::<module>::events::<EventName>` — with `match_and_decode(log) -> Option<Self>`
* `abi::<module>::functions::<FunctionName>` — callable via `eth_call` / `RpcBatch`

**`<module>` comes from the `write_to_file` filename, not from the `Abigen::new` name.** That first argument is declared `_contract_name` and is discarded — it is documentation, nothing more. So:

```rust
Abigen::new("UniswapV3Pool", "abi/uniswap_v3_pool.json")   // ← this string is ignored
    .write_to_file("src/abi/uniswap_v3_pool.rs")           // ← this decides the path
// → abi::uniswap_v3_pool::events::Swap     (NOT abi::UniswapV3Pool::…)
```

Name the file in `snake_case` and declare it in `src/abi/mod.rs` with the same name.

Field names are converted to `snake_case`: Solidity `amount0In` becomes `swap.amount0_in`. Integer params arrive as `substreams::scalar::BigInt`, addresses as `Vec<u8>`.

### Decoding

```rust
use substreams_ethereum::Event;   // REQUIRED — without it, no .match_and_decode()

for (log, _call) in trx.logs_with_calls() {
    if let Some(swap) = abi::uniswap_v3_pool::events::Swap::match_and_decode(log) {
        // swap.amount0: BigInt (signed for V3)
        // swap.sender:  Vec<u8>
    }
}
```

`match_and_decode` verifies topic0 **and** decodes, returning `None` on mismatch. Chain several with `else if` to handle multiple event types in one pass:

```rust
if let Some(e) = Swap::match_and_decode(log) {
    // …
} else if let Some(e) = Mint::match_and_decode(log) {
    // …
}
```

### The concise form: `block.events()`

When a handler processes exactly **one** event type, `block.events::<E>(&addresses)` replaces the tx→log nest with a single loop. It filters by contract address, decodes, and yields `(event, log)` pairs:

```rust
const USDC: [u8; 20] = hex_literal::hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");

// addresses is &[&[u8]]
// Hex::encode has no 0x — re-prefix for emitted address strings.
for (transfer, log) in block.events::<abi::erc20::events::Transfer>(&[&USDC]) {
    out.push(Transfer {
        from: format!("0x{}", Hex::encode(&transfer.from)),
        to: format!("0x{}", Hex::encode(&transfer.to)),
        amount: transfer.value.to_string(),
        log_index: log.index(),          // LogView accessor, not a field
    });
}
```

Note it iterates **receipt logs** of successful transactions, so it does not hand you the originating `Call`. Use `trx.logs_with_calls()` when you need call context (caller, depth) alongside the log. For multiple event types, fall back to the nested loop with `else if` chains above — `events::<E>` is monomorphic in `E`.

### Where to get the ABI

1. Etherscan → contract → *Contract ABI* (verified contracts only).
2. The project's own repo / npm package (e.g. `@uniswap/v3-core/artifacts`).
3. `substreams init` → `ethereum-minimal` fetches it for a verified address.

A **minimal ABI is fine and preferable** — it only needs the events and functions you actually use. A hand-written 3-entry ABI for `symbol`/`decimals`/`Transfer` compiles faster and is easier to review than a 400-entry dump.

## Path 2 — Solidity source, no ABI JSON (T6.1)

Real-world case: unverified or legacy contracts. Derive topic0 from the source.

```solidity
event Swap(
    address indexed sender,
    uint amount0In,
    uint amount1In,
    uint amount0Out,
    uint amount1Out,
    address indexed to
);
```

1. Strip parameter names, `indexed`, and whitespace.
2. Expand aliases: `uint` → `uint256`, `int` → `int256`, `byte` → `bytes1`.
3. Canonical: `Swap(address,uint256,uint256,uint256,uint256,address)`
4. `topic0 = keccak256(canonical)` → `d78ad95f…`

```bash
cast keccak "Swap(address,uint256,uint256,uint256,uint256,address)"
```

Then decode by hand — see *Indexed vs non-indexed* below.

## Path 3 — Known signature

For ERC-20/721/1155 and other standards, use the verified constants in [common-contracts.md](./common-contracts.md) with `ethabi` for the data words.

## Indexed vs non-indexed — the core decoding rule

This is what agents most often get wrong.

| Slot | Contents |
|---|---|
| `topics[0]` | `keccak256(canonical signature)` — absent for `anonymous` events |
| `topics[1..]` | **indexed** params, in declaration order, one 32-byte word each — **max 3** |
| `data` | **non-indexed** params, ABI-encoded, concatenated 32-byte words |

Indexed and non-indexed params interleave in the declaration but are stored in **two separate places**, each keeping its own relative order. For the V2 `Swap` above: `sender` and `to` are indexed (`topics[1]`, `topics[2]`) while the four amounts are non-indexed (`data`, in order).

### Decoding a topic address

Addresses are left-padded to 32 bytes:

```rust
fn topic_to_address(topic: &[u8]) -> String {
    format!("0x{}", hex::encode(&topic[12..32]))
}
```

### Decoding data words

```rust
use substreams::scalar::BigInt;

// data is 4 × 32-byte words: [amount0In][amount1In][amount0Out][amount1Out]
fn word(data: &[u8], i: usize) -> BigInt {
    BigInt::from_unsigned_bytes_be(&data[i * 32..(i + 1) * 32])
}

let amount0_in = word(&log.data, 0);
let amount1_in = word(&log.data, 1);
```

Use `BigInt::from_signed_bytes_be` for `int256`/`int128` params (Uniswap V3 amounts), otherwise negative values decode as astronomically large positives.

With `ethabi` for anything non-trivial (dynamic types, arrays, structs):

```rust
use ethabi::{decode, ParamType};
use substreams::errors::Error;   // alias for anyhow::Error — no extra dependency needed

let decoded = decode(
    &[ParamType::Uint(256), ParamType::Uint(256)],
    &log.data,
).map_err(|e| Error::msg(format!("decode failed: {e}")))?;
```

Requires `ethabi = "17"` in `Cargo.toml` — **not `18`**, which duplicates the stack `substreams-ethereum-core` already links (see SKILL.md). Don't reach for `anyhow::anyhow!` here unless you also declare `anyhow` yourself; `substreams::errors::Error` is already an `anyhow::Error` alias and needs no new dependency.

Note `ethabi` is only optional on the **hand-decode** path — T6.1 rolls its own `uint256`→decimal conversion and declares no `ethabi`. On the **Abigen** path it is mandatory regardless of whether you call it yourself, because the generated module references it.

### Gotchas

* **Indexed dynamic types are hashed, not stored.** An `indexed string` or `indexed bytes` topic holds `keccak256(value)` — the original is **unrecoverable** from the log. If the user needs that value, it must come from the non-indexed data or an RPC call. Do not emit the hash as if it were the string.
* **Anonymous events have no topic0**; `topics[0]` is the first indexed param. Match on address + `topics.len()` instead.
* **Overloaded events** (same name, different params) have different topic0. Match the exact signature.
* **`log.data.len() % 32 != 0`** means you have the wrong event or a dynamic type — do not silently pad.

## Filtering strategy

Order filters cheapest-first:

```rust
for (log, _call) in trx.logs_with_calls() {
    if log.address != POOL { continue; }                      // 1. 20-byte compare
    if log.topics.is_empty() || log.topics[0] != SWAP { continue; }  // 2. 32-byte compare
    let swap = match Swap::match_and_decode(log) {            // 3. full decode
        Some(s) => s,
        None => continue,
    };
}
```

For **many** addresses, use a `HashSet<Vec<u8>>` built once outside the loop, or an **index module** emitting `contract:<hex>` keys so consumers skip empty blocks entirely. For dynamically discovered contracts (factory-created pools), track them in a `set_if_not_exists` store keyed by address and read it in the consuming map — see [rpc-and-tokens.md](./rpc-and-tokens.md) and the factory pattern in `substreams-dev` `references/patterns.md`.

## Build troubleshooting

| Error | Cause |
|---|---|
| `no method named 'decode'/'match_and_decode' found` | Missing `use substreams_ethereum::Event;` |
| `cannot find Abigen in substreams_ethereum` (build.rs) | Missing `substreams-ethereum` in `[build-dependencies]` |
| `failed to load ABI` | Bad path (relative to crate root) or the file is an Etherscan *page*, not raw JSON |
| `file not found for module 'abi'` | Missing `src/abi/mod.rs` or `mod abi;` in `lib.rs` |
| Generated file not refreshed | `build.rs` reruns on ABI change; force with `cargo clean -p <crate>` |
| `hex_literal` unresolved | Cargo key is `hex-literal` (hyphen); import is `hex_literal` (underscore) |
