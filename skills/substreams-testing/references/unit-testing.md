# Unit Testing Guide

Unit tests exercise Substreams handlers and pure helpers **without** the WASM runtime or network.

## Goals

- **Fast** — milliseconds per test
- **Isolated** — no Firehose / `substreams run` in the default suite
- **Deterministic** — fixtures and builders only
- **Comprehensive** — empty blocks, bad logs, max values, chained maps

## `substreams::testing` (0.7.4+)

Module is **`#[cfg(test)]` only** and marked **experimental**. Public helpers:

| Helper | Purpose |
|---|---|
| `map!(handler(args…))` | Calls generated `__impl_<handler>` |
| `clock("…")` | Builds a `Clock` for handlers that take clock input |

There is **no** store mock API. Test pure key/value logic; exercise stores via CLI (`substreams run`, `--test-file`).

### Cargo.toml

```toml
[dependencies]
substreams = "0.7"
# chain package as needed:
substreams-ethereum = "0.11"
# substreams-solana = "0.15"

[dev-dependencies]
hex = "0.4"
base64 = "0.22"
prost = "0.13"
prost-types = "0.13"
# optional:
# proptest = "1"
# criterion = { version = "0.5", features = ["html_reports"] }
```

Handlers must be testable (default). Opt out only when needed:

```rust
#[substreams::handlers::map(no_testable)]
pub fn my_handler(block: Block) -> Result<Output, Error> {
    // no __impl_my_handler — extract pure logic for unit tests
}
```

### `map!` patterns

```rust
use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;

#[substreams::handlers::map]
pub fn map_events(block: Block) -> Result<Events, Error> {
    let events = block
        .logs()
        .filter(|log| !log.topics.is_empty())
        .filter_map(|log| parse_event(log).ok())
        .collect();
    Ok(Events { events })
}

#[substreams::handlers::map]
pub fn filter_events(event_type: String, events: Events) -> Result<Events, Error> {
    let events = events
        .events
        .into_iter()
        .filter(|e| e.event_type == event_type)
        .collect();
    Ok(Events { events })
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams::testing;

    #[test]
    fn empty_block() {
        let out = testing::map!(map_events(Block::default())).unwrap();
        assert!(out.events.is_empty());
    }

    #[test]
    fn chained() {
        let block = create_block_with_transfer();
        let all = testing::map!(map_events(block)).unwrap();
        let transfers =
            testing::map!(filter_events("transfer".to_string(), all)).unwrap();
        assert!(transfers.events.iter().all(|e| e.event_type == "transfer"));
    }
}
```

`map!(name(args))` expands to `__impl_name(args)`.

### `clock` — string required

```rust
use substreams::testing::clock;

#[test]
fn clock_examples() {
    let c = clock("12345");
    assert_eq!(c.number, 12345);
    assert_eq!(c.id, "12345");

    // @value is milliseconds since Unix epoch
    let c = clock("50@1609459200000");
    assert_eq!(c.number, 50);
    assert_eq!(c.timestamp.as_ref().unwrap().seconds, 1_609_459_200);

    let c = clock("blockhash@1609459200500");
    assert_eq!(c.number, 0);
    assert_eq!(c.id, "blockhash");
}
```

Handlers that take `Clock` as an input:

```rust
#[substreams::handlers::map]
pub fn map_with_clock(clock: Clock, block: Block) -> Result<Events, Error> {
    // …
}

#[test]
fn test_with_clock() {
    let clk = substreams::testing::clock("17000000@1680000000000");
    let block = Block {
        number: 17_000_000,
        ..Default::default()
    };
    let _ = substreams::testing::map!(map_with_clock(clk, block)).unwrap();
}
```

### Legacy wrapper pattern

For older SDKs or `no_testable`:

```rust
#[substreams::handlers::map]
pub fn map_events(block: Block) -> Result<Events, Error> {
    map_events_impl(block)
}

pub fn map_events_impl(block: Block) -> Result<Events, Error> {
    // real logic
}

#[test]
fn legacy() {
    let out = map_events_impl(Block::default()).unwrap();
    assert!(out.events.is_empty());
}
```

## Ethereum block construction

### Correct field model

`Block` fields (not exhaustive): `hash`, `number`, `size`, `header`, `uncles`, `transaction_traces`, `balance_changes`, `code_changes`, `system_calls`, `detail_level`, `ver`.

- **`timestamp_seconds()`** — method reading `header.timestamp`, not a field
- **`parent_hash`** — on `BlockHeader`, not on `Block`

```rust
use prost_types::Timestamp;
use substreams_ethereum::pb::eth::v2::{
    Block, BlockHeader, Log, TransactionReceipt, TransactionTrace,
};

pub fn create_block_with_transfer() -> Block {
    Block {
        number: 17_000_000,
        hash: bytes32(0xaa),
        header: Some(BlockHeader {
            parent_hash: bytes32(0xbb),
            number: 17_000_000,
            timestamp: Some(Timestamp {
                seconds: 1_680_000_000,
                nanos: 0,
            }),
            ..Default::default()
        }),
        transaction_traces: vec![TransactionTrace {
            hash: bytes32(0xcc),
            status: 1, // SUCCEEDED — filter failed txs if your module does
            receipt: Some(TransactionReceipt {
                logs: vec![erc20_transfer_log(
                    "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "742d35cc6634c0532925a3b844bc454e4438f44e",
                    "5aaeb6053f3e94c9b9a09f33669435e7ef1beaed",
                    1_000_000u128, // smallest units
                )],
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn bytes32(fill: u8) -> Vec<u8> {
    vec![fill; 32]
}

fn pad_address_topic(addr_hex_no_0x: &str) -> Vec<u8> {
    // 32-byte left-padded address topic
    hex::decode(format!("{:0>64}", addr_hex_no_0x)).expect("addr hex")
}

fn u256_word(amount: u128) -> Vec<u8> {
    // 32-byte big-endian; for larger amounts use a proper U256 encoder
    let mut out = vec![0u8; 32];
    out[16..].copy_from_slice(&amount.to_be_bytes());
    out
}

fn erc20_transfer_log(contract: &str, from: &str, to: &str, amount: u128) -> Log {
    Log {
        address: hex::decode(contract).expect("contract"),
        topics: vec![
            hex::decode("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
                .unwrap(),
            pad_address_topic(from),
            pad_address_topic(to),
        ],
        data: u256_word(amount),
        ..Default::default()
    }
}
```

**Do not** `hex::decode` a decimal amount string padded with `{:0>64}` — that is not a big-endian integer encoding.

### Builder pattern

```rust
pub struct TestBlockBuilder {
    block: Block,
}

impl TestBlockBuilder {
    pub fn new(number: u64) -> Self {
        Self {
            block: Block {
                number,
                hash: {
                    let mut h = vec![0u8; 32];
                    h[24..].copy_from_slice(&number.to_be_bytes());
                    h
                },
                header: Some(BlockHeader {
                    number,
                    timestamp: Some(Timestamp {
                        seconds: 1_680_000_000 + (number as i64) * 12,
                        nanos: 0,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        }
    }

    pub fn with_timestamp_secs(mut self, seconds: i64) -> Self {
        if let Some(h) = self.block.header.as_mut() {
            h.timestamp = Some(Timestamp { seconds, nanos: 0 });
        }
        self
    }

    pub fn add_log(mut self, log: Log) -> Self {
        self.block.transaction_traces.push(TransactionTrace {
            hash: bytes32(0x11),
            status: 1,
            receipt: Some(TransactionReceipt {
                logs: vec![log],
                ..Default::default()
            }),
            ..Default::default()
        });
        self
    }

    pub fn build(self) -> Block {
        self.block
    }
}
```

## Fixtures from Firehose (protobuf)

```bash
firecore tools firehose-single-block-client \
  mainnet.eth.streamingfast.io:443 17000000 \
  -o bytes --bytes-encoding=base64 \
  > src/testdata/eth_mainnet_17000000.binpb.b64
```

```rust
fn load_block_b64(path: &str) -> Block {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let raw = std::fs::read_to_string(path).expect("fixture");
    let bytes = STANDARD.decode(raw.trim()).expect("base64");
    Block::decode(bytes.as_slice()).expect("Block decode")
}

#[test]
fn real_block_smoke() {
    let block = load_block_b64("src/testdata/eth_mainnet_17000000.binpb.b64");
    let out = substreams::testing::map!(map_events(block)).unwrap();
    // assert against known content of that block
    let _ = out;
}
```

Commit small fixtures under `src/testdata/` or `tests/fixtures/`. Prefer binary/base64 protobuf over JSON round-trips (JSON field shapes from firecore are not always 1:1 with `prost` types).

## Solana unit tests

Same `map!` approach with `substreams_solana` types:

- Build or load a `Block` / confirmed block fixture
- Prefer typed instruction decode helpers from `substreams-solana`
- Assert structured proto fields, not raw instruction bytes

## What to assert

| Case | Expectation |
|---|---|
| Empty block | Empty output, `Ok` |
| Missing topics / short data | `Err` or skip, never panic |
| Max uint256 amount | Parses or explicit overflow error |
| Failed tx (`status != 1`) | Honors your product rule (include/skip) |
| Address encoding | Consistent hex length / `0x` / lower-case policy |
| Chained maps | Filter preserves invariants |

### Property-style checks (optional)

```rust
// conceptual — use proptest / quickcheck if you add the deps
// For all synthetic transfers you can encode, parse(encode(t)) == t
```

Keep generators producing **valid** topic/data layouts; random hex often only tests “returns Err”.

## Store-related unit tests

Without a runtime store:

```rust
fn balance_key(token: &str, owner: &str) -> String {
    format!("{}:{}", token, owner)
}

#[test]
fn key_format_stable() {
    assert_eq!(
        balance_key("0xusdc", "0xalice"),
        "0xusdc:0xalice"
    );
}
```

Validate store **deltas** and multi-block consistency with `substreams run` and `--test-file` (see integration guide).

## Running tests

```bash
cargo test --lib
cargo test --lib map_events -- --nocapture
# network / CLI tests should be #[ignore]
cargo test -- --ignored
```

WASM target is **not** required for pure `map!` unit tests; still run `substreams build` in CI to catch WASM link issues.

## See also

- [Integration testing](./integration-testing.md) — CLI, `--test-file`, multi-block
- [Firecore tools](./firecore-tools.md) — fetching fixtures
- [Official testing docs](https://docs.substreams.dev/reference-material/development-tools/testing)
