---
name: substreams-testing
description: Expert knowledge for testing Substreams — unit tests with substreams::testing (map!, clock), real block fixtures via firecore, CLI integration (substreams run, --test-file), performance checks, and CI patterns.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, opencode, vscode, windsurf]
metadata:
  version: 1.6.0
  author: StreamingFast
  documentation: https://docs.substreams.dev/reference-material/development-tools/testing
---

# Substreams Testing Expert

Expert assistant for testing Substreams modules: unit tests without WASM, real-block fixtures, CLI integration/golden checks, and CI.

## When to use

Load this skill when the user wants to **test**, **verify**, **benchmark**, or **CI** a Substreams project. For authoring modules, prefer `substreams-dev` plus the chain skill (`substreams-ethereum` / `substreams-solana`).

## Why test

- Blockchain data is immutable — bugs compound across millions of blocks
- High throughput amplifies small mistakes
- Reorgs and sparse contracts create edge cases synthetic data misses
- DeFi and sinks depend on correct amounts, addresses, and keys

## Testing pyramid (agent default)

| Level | Goal | Primary tools |
|---|---|---|
| **Unit** | Pure handler logic, edge cases | `cargo test` + `substreams::testing::map!` |
| **Integration** | Real blocks / multi-module behavior | Fixtures + `map!`, or `substreams run` |
| **CLI / golden** | Engine + WASM + network data | `substreams run -o jsonl`, `--test-file` |
| **Performance** | Scale / production mode | Criterion, `time substreams run --production-mode` |

**Agent workflow (match this repo’s eval style):**

1. `cargo test` (unit + pure helpers)
2. `substreams build`
3. `substreams run -s <start> -t +N <module> -o jsonl` on a small range
4. Optional: `--test-file tests/assertions.yaml` or diff against a golden JSONL
5. Only then widen the range or enable `--production-mode`

Always pin `initialBlock` near the test window (see `substreams-dev`) so cold runs do not backfill from genesis.

## Unit testing (`substreams::testing`, 0.7.4+)

**Experimental API** — only available under `#[cfg(test)]`. Surface today: **`map!`** and **`clock`**. There is no official store mock / `TestStore`.

### `map!` — call handlers without WASM

`#[substreams::handlers::map]` generates a testable `__impl_<name>` function. `map!` rewrites the call to that function:

```rust
use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;

#[substreams::handlers::map]
pub fn map_events(block: Block) -> Result<Events, Error> {
    let events = block
        .logs()
        .filter(|log| is_relevant(log))
        .filter_map(|log| parse_event(log).ok())
        .collect();
    Ok(Events { events })
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams::testing;

    #[test]
    fn empty_block_ok() {
        let result = testing::map!(map_events(Block::default())).unwrap();
        assert!(result.events.is_empty());
    }

    #[test]
    fn chain_handlers() {
        let block = create_test_block();
        let all = testing::map!(map_events(block)).unwrap();
        let filtered =
            testing::map!(filter_events("transfer".to_string(), all)).unwrap();
        assert!(filtered.events.iter().all(|e| e.event_type == "transfer"));
    }
}
```

Opt out: `#[substreams::handlers::map(no_testable)]` (use a thin pure helper for tests instead).

**Legacy:** extract `_map_events(block) -> Result<...>` and call it from both the handler and tests when you are below 0.7.4 or use `no_testable`.

### `clock` — requires a string

Signature: `clock(input: impl AsRef<str>) -> Clock`.

```rust
use substreams::testing::clock;

let c = clock("17000000");                 // number + id from digits; ts ≈ number (seconds)
let c = clock("17000000@1680000000000");   // explicit ms-since-epoch after @
let c = clock("abc@1609459200500");        // non-numeric id → number 0
```

Do **not** call `clock()` with zero arguments — that does not compile.

### Constructing Ethereum blocks (correct fields)

`substreams_ethereum::pb::eth::v2::Block` fields include `hash`, `number`, `header`, `transaction_traces`, …  
There is **no** `timestamp_seconds` or `parent_hash` field on `Block` — those live on **`header`** (`timestamp_seconds()` is a **method**).

```rust
use prost_types::Timestamp;
use substreams_ethereum::pb::eth::v2::{
    Block, BlockHeader, Log, TransactionReceipt, TransactionTrace,
};

fn create_block_with_transfer() -> Block {
    Block {
        number: 17_000_000,
        hash: hex::decode("aa".repeat(32)).unwrap(),
        header: Some(BlockHeader {
            parent_hash: hex::decode("bb".repeat(32)).unwrap(),
            timestamp: Some(Timestamp {
                seconds: 1_680_000_000,
                nanos: 0,
            }),
            ..Default::default()
        }),
        transaction_traces: vec![TransactionTrace {
            hash: hex::decode("cc".repeat(32)).unwrap(),
            receipt: Some(TransactionReceipt {
                logs: vec![Log {
                    address: hex::decode("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                        .unwrap(),
                    topics: vec![
                        // Transfer(address,address,uint256)
                        hex::decode(
                            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                        )
                        .unwrap(),
                        // from / to left-padded to 32 bytes
                        hex::decode(format!("{:0>64}", "742d35cc6634c0532925a3b844bc454e4438f44e")).unwrap(),
                        hex::decode(format!("{:0>64}", "5aaeb6053f3e94c9b9a09f33669435e7ef1beaed")).unwrap(),
                    ],
                    // amount as 32-byte big-endian hex, not a decimal string
                    data: hex::decode(format!("{:0>64x}", 1_000_000u64)).unwrap(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    }
}
```

**Solana:** same `map!` pattern with `substreams_solana` block types — keep decoding rules from `substreams-solana` (typed instruction fields, not raw blobs).

### Store modules

There is **no** supported in-process store mock in `substreams::testing`. Prefer:

- Unit-test pure functions that compute keys/values/deltas
- Exercise stores with `substreams run` and/or **`--test-file`** (debug store outputs)

## Real block fixtures (Firecore)

Prefer **protobuf bytes** fixtures over hand-rolled JSON of full blocks.

```bash
# Single block as base64 protobuf (best for unit/integration fixtures)
firecore tools firehose-single-block-client \
  mainnet.eth.streamingfast.io:443 17000000 \
  -o bytes --bytes-encoding=base64 \
  > src/testdata/eth_mainnet_17000000.binpb.b64

# Range as JSONL (inspection / golden drafting — not ideal to re-decode as Block)
firecore tools firehose-client mainnet -o json -- 17000000 +100
```

Load in Rust:

```rust
use prost::Message;
use substreams_ethereum::pb::eth::v2::Block;

fn load_block_b64(path: &str) -> Block {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let b64 = std::fs::read_to_string(path).expect("fixture missing");
    let bytes = STANDARD.decode(b64.trim()).expect("base64");
    Block::decode(bytes.as_slice()).expect("protobuf Block")
}
```

Auth: data-plane token via `substreams auth` / `SUBSTREAMS_API_TOKEN` (or Firehose env vars the CLI documents). Not Portal admin auth.

Details: [references/firecore-tools.md](./references/firecore-tools.md)

## CLI integration and golden output

```bash
substreams build
substreams run -s 17000000 -t +100 map_events --network mainnet -o jsonl > /tmp/out.jsonl
test -s /tmp/out.jsonl
```

Wrap in Rust with `std::process::Command` only for optional/nightly tests (`#[ignore]`), so default `cargo test` stays offline-friendly.

**Production mode:** re-run the same range with `--production-mode` and confirm outputs match development mode (caching / parallel path).

### `substreams run --test-file` (jq assertions)

Built-in assertion runner. Spec formats: **`.yaml`**, **`.jsonl`**, **`.csv`**.

Each case: `module`, `block`, `path` (gojq), `expect`, optional `op` / `args`.

```yaml
# tests/assertions.yaml
tests:
  - module: map_events
    block: 17000000
    path: .events | length
    expect: "0"
    op: float
  - module: map_events
    block: 17000000
    path: .events[0].contract
    expect: "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
  - module: store_balances
    block: 17000010
    path: select(.key == "token:alice") | .new
    expect: "1000"
```

```bash
substreams run substreams.yaml map_events \
  -s 17000000 -t +20 \
  --test-file tests/assertions.yaml \
  --test-verbose
```

Store assertions need store outputs in the stream (debug stores / module selection as required by your CLI version). Prefer absolute block numbers that fall inside `-s`/`-t`.

## Performance (short)

- Benchmark pure map logic with **Criterion** on real heavy fixtures (`cargo bench`)
- Time CLI: `time substreams run -s … -t +1000 … --production-mode`
- Watch OOM / kill signals on large ranges; add `blockFilter` indexes before micro-optimizing (see `substreams-dev`)

Full guide: [references/performance-testing.md](./references/performance-testing.md)

## CI sketch

```yaml
# .github/workflows/test.yml (excerpt)
jobs:
  unit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
          components: rustfmt, clippy
      - run: cargo test --lib
      - run: cargo clippy -- -D warnings
      - run: cargo fmt -- --check
      - run: substreams build   # after installing substreams CLI

  # Optional job with secrets: SUBSTREAMS_API_TOKEN
  # integration:
  #   run: |
  #     substreams run -s 17000000 -t +50 map_events --network mainnet -o jsonl
  #     substreams run … --test-file tests/assertions.yaml
```

Do not call hosted endpoints from default PR jobs without secrets and rate limits.

## Best practices

**Do**

- Unit-test parsers and filters with synthetic + real fixtures
- Keep default tests offline; mark network tests `#[ignore]`
- Encode log amounts as **32-byte hex big-endian**, not decimal strings
- Assert on golden JSONL or `--test-file` for regressions
- Test empty blocks, malformed logs, max `uint256`, and address casing conventions you emit

**Don’t**

- Invent store mocks that “simulate” Substreams delta/undo semantics
- Set fake fields on `Block` that do not exist (`timestamp_seconds`, `parent_hash` at top level)
- Rely only on happy-path single-block tests
- Pin `initialBlock` to protocol genesis for local 100-block checks

## Resources

* [Official testing guide](https://docs.substreams.dev/reference-material/development-tools/testing)
* [Unit testing](./references/unit-testing.md)
* [Integration testing](./references/integration-testing.md)
* [Performance testing](./references/performance-testing.md)
* [Firecore tools](./references/firecore-tools.md)
* Discord: https://discord.gg/streamingfast
* Issues: https://github.com/streamingfast/substreams/issues
