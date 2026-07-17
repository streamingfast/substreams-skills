# Subgraph → Substreams Conversion Guide

Load this file when converting a subgraph (The Graph) into a Substreams pipeline.

## Conceptual Mapping

| Subgraph Concept | Substreams Equivalent |
|---|---|
| `schema.graphql` entity | Protobuf message (`.proto`) |
| `subgraph.yaml` data source | `substreams.yaml` module + `initialBlock` |
| AssemblyScript event handler | Rust `map` module |
| Entity store (auto) | Explicit `store` module |
| `graph-node` indexer | **SQL sink (`db_out`)** for greenfield; `EntityChanges` / `graph_out` only if you still need Graph Node (see §5, optional/legacy) |
| Template (dynamic data source) | Map module emitting new addresses → store |

## Step-by-Step Migration

### 1. Convert the GraphQL Schema to Protobuf

**Before (GraphQL SDL):**

```graphql
type Transfer @entity {
  id: ID!
  from: String!
  to: String!
  amount: BigDecimal!
  blockNumber: BigInt!
  timestamp: BigInt!
}
```

**After (Protobuf):**

```protobuf
syntax = "proto3";
package myproject.v1;

message Transfer {
  string id = 1;
  string from = 2;
  string to = 3;
  string amount = 4;       // BigDecimal as string
  uint64 block_number = 5;
  uint64 timestamp = 6;
}

message Transfers {
  repeated Transfer transfers = 1;
}
```

> **BigInt / BigDecimal in Protobuf**: Substreams does not have native BigDecimal. Represent them as `string` (human-readable decimal) or `bytes` (big-endian). Use the `substreams::scalar::BigDecimal` / `BigInt` helpers in Rust:
>
> ```rust
> use substreams::scalar::{BigDecimal, BigInt};
>
> // From a raw u256 value (e.g. from event.value which is substreams BigInt):
> let amount_str = event.value.to_decimal(6).to_string();  // USDC (6 decimals)
>
> // Parse back from string:
> let amount: BigDecimal = "1234.56".parse().unwrap_or_default();
>
> // BigInt arithmetic:
> let raw: BigInt = event.amount; // already BigInt from ABI codegen
> let human = raw.to_decimal(18).to_string(); // 18-decimal token
> ```

### 2. Identify the Data Source Chain and Initial Block

**subgraph.yaml:**
```yaml
dataSources:
  - kind: ethereum/contract
    name: ERC20Token
    network: mainnet
    source:
      address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
      abi: ERC20
      startBlock: 6082465
```

**substreams.yaml:**
```yaml
specVersion: v0.1.0
package:
  name: erc20_transfers
  version: v0.1.0
  url: https://github.com/myorg/erc20_transfers   # set it — avoids package.url warning
  description: ERC20 transfer events indexer        # set it — avoids package.description warning
network: mainnet          # same as subgraph network

protobuf:
  files:
    - transfers.proto
  importPaths:
    - ./proto

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/erc20_transfers.wasm

modules:
  - name: map_transfers
    kind: map
    initialBlock: 6082465   # from subgraph startBlock
    inputs:
      - source: sf.ethereum.type.v2.Block
    output:
      type: proto:myproject.v1.Transfers
```

> **IMPORTANT**: Use the contract's `startBlock` from the subgraph as `initialBlock` in Substreams. Do NOT use block 0 or the chain genesis block — this forces a full chain backfill from the beginning.

### 3. Convert Event Handlers

**Before (AssemblyScript):**

```typescript
export function handleTransfer(event: TransferEvent): void {
  let transfer = new Transfer(
    event.transaction.hash.toHexString() + "-" + event.logIndex.toString()
  );
  transfer.from = event.params.from.toHexString();
  transfer.to = event.params.to.toHexString();
  transfer.amount = event.params.value.toBigDecimal();
  transfer.blockNumber = event.block.number;
  transfer.timestamp = event.block.timestamp;
  transfer.save();
}
```

**After (Rust, Substreams map module):**

> **Ethereum address filtering — use the foundational `filtered_events` module (block filter)**
>
> Iterating `block.logs()` and filtering in-handler still processes every block. Use
> `ethereum_common`'s `filtered_events` module instead: it applies a **block-level skip**
> (blocks with no matching logs are never decoded or executed) **and** returns only the
> matching events — so your handler processes far fewer blocks and pays far less.
>
> ```yaml
> imports:
>   # Full spkg URL — short form ethereum_common@v0.3.3 does not resolve via the CLI
>   eth_common: https://spkg.io/v1/packages/ethereum-common/v0.3.3
>
> modules:
>   - name: map_transfers
>     kind: map
>     initialBlock: 6082465   # from subgraph startBlock
>     inputs:
>       - map: eth_common:filtered_events   # block-skipped + event-filtered for you
>     output:
>       type: proto:myproject.v1.Transfers
>
> params:
>   # REQUIRED — must override the foundational default or you silently emit wrong data.
>   # Use 0x-prefixed lowercase hex (EVM checksum/mixed-case addresses will not match).
>   eth_common:filtered_events: "evt_addr:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
> ```
>
> `ethereum-common` v0.3.3 also provides `filtered_calls`, `filtered_transactions`, and
> `filtered_events_and_calls`. For the full block-filtering guide including the SQE query
> syntax, rolling a custom `blockIndex`, and Solana's `solana_common` equivalents, see
> the `substreams-dev` skill's `references/block-filtering.md`.
>
> The in-handler address check shown below is a fallback for when no foundational
> `filtered_*` module covers your use case. When depending on `eth_common:filtered_events`,
> you do **not** need an in-handler address check — the module has already filtered for you.
>
> **Decoding quality (EVM):** prefer the `substreams-ethereum` skill loop —
> `block.transactions()` + `trx.logs_with_calls()` + Abigen `match_and_decode` — or hand off
> entirely to that skill. `block.logs()` is a shorter fallback; `logs_with_calls()` excludes
> reverted sub-calls and is the production default.

```rust
use substreams::errors::Error;
use substreams::Hex;
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::Event;

use crate::abi::erc20::events::Transfer as TransferEvent;
use crate::pb::myproject::v1::{Transfer, Transfers};

// Hex::encode is lowercase and has NO 0x — always re-prefix for emitted addresses/tx hashes.
fn hex0x(b: &[u8]) -> String {
    format!("0x{}", Hex::encode(b))
}

// Contract address (from subgraph.yaml source.address)
const TOKEN_ADDRESS: [u8; 20] =
    hex_literal::hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

#[substreams::handlers::map]
pub fn map_transfers(block: eth::Block) -> Result<Transfers, Error> {
    let mut transfers = Transfers::default();

    for trx in block.transactions() {
        // successful txs only
        let tx_hash = hex0x(&trx.hash);

        for (log, _call) in trx.logs_with_calls() {
            // Filter by contract address (replaces subgraph data source address filter).
            // In production, prefer a foundational filtered_events module as input
            // so blocks with no matching logs are skipped entirely (see note above).
            if log.address != TOKEN_ADDRESS {
                continue;
            }

            // Decode event (replaces AssemblyScript event handler binding)
            if let Some(event) = TransferEvent::match_and_decode(log) {
                let id = format!("{}-{}", tx_hash, log.index);

                transfers.transfers.push(Transfer {
                    id,
                    from: hex0x(&event.from),
                    to: hex0x(&event.to),
                    // 6 = USDC decimals; parameterize per token — never hardcode 18
                    amount: event.value.to_decimal(6).to_string(),
                    block_number: block.number,
                    timestamp: block.timestamp_seconds(),
                });
            }
        }
    }

    Ok(transfers)
}
```

> **`substreams_ethereum::Event`** provides `match_and_decode` — equivalent to subgraph's automatic event binding. Add `substreams-ethereum = "0.11"` to `Cargo.toml`. Full Abigen / `logs_with_calls` / raw-topic0 patterns → **`substreams-ethereum` skill**.

> **Multiple contract addresses**: Subgraphs commonly index many contracts (e.g. all pairs in a factory). In Substreams, use a `HashSet` of known addresses populated from a store, then check membership against `log.address`. For dynamically discovered addresses (factory pattern), see Section 6 (Dynamic Data Sources).

```rust
// For a fixed allow-list of contracts:
const KNOWN_TOKENS: &[[u8; 20]] = &[
    hex_literal::hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // USDC
    hex_literal::hex!("dAC17F958D2ee523a2206206994597C13D831ec7"), // USDT
];

// In handler:
if !KNOWN_TOKENS.contains(&log.address) {
    continue;
}
```

### 4. Migrating Entity Storage — SQL Sink (Postgres / ClickHouse)

The recommended way to persist subgraph entity data in Substreams is the **SQL sink** (Postgres or ClickHouse), using the `db_out` module pattern with `DatabaseChanges`. This replaces subgraph's auto-managed entity store.

> **Engine choice (one-liner):** **mutable balances / UPDATE / DELETE / upsert / delta ops → PostgreSQL + Database Changes.** ClickHouse (and from-proto on either engine) is **insert-only** — fine for event mirrors, not for current-state entity rows. Details → **`substreams-sql`**.

> **`store` modules** are still used in Substreams, but for a specific purpose: caching intermediate state that other modules need to read during processing (e.g. tracking dynamically discovered contract addresses — see Section 6). They are not the replacement for subgraph entity persistence. Use `db_out` + SQL sink for that.

#### Step 4a — Define the SQL schema

Create a `schema.sql` that mirrors your subgraph's `schema.graphql` entities:

**Before (GraphQL SDL):**
```graphql
type Transfer @entity {
  id: ID!
  from: String!
  to: String!
  amount: BigDecimal!
}

type Balance @entity {
  id: ID!
  address: String!
  amount: BigDecimal!
}
```

**After (`schema.sql`):**
```sql
CREATE TABLE IF NOT EXISTS transfers (
    id         TEXT NOT NULL,
    from_addr  TEXT NOT NULL,
    to_addr    TEXT NOT NULL,
    amount     NUMERIC NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS balances (
    id         TEXT NOT NULL,
    address    TEXT NOT NULL,
    amount     NUMERIC NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);
```

#### Step 4b — Add `substreams-database-change` and emit `DatabaseChanges`

Add to `Cargo.toml`:
```toml
substreams-database-change = "4"
```

Add a `db_out` map module that converts your map output into database change records.

**There is no auto-CREATE on `update_row`.** Pick the operation explicitly:

| Need | API |
|---|---|
| Insert a new row (fail / no-op if exists — CREATE op) | `create_row` |
| Update an existing row (UPDATE op; row must already exist) | `update_row` |
| Insert or update | `upsert_row` |
| Atomic in-DB delta (`COALESCE(col,0) + x`) — **Postgres only** | `upsert_row` + `.add` / `.sub` / `.max` / … |

Use `.set(...)` for field values (including numeric strings). There is **no** `set_bigdecimal`. Canonical Rust path for the proto type:

```rust
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_database_change::tables::Tables;

#[substreams::handlers::map]
pub fn db_out(
    transfers: Transfers,
) -> Result<DatabaseChanges, substreams::errors::Error> {
    let mut tables = Tables::new();

    for transfer in transfers.transfers {
        // Event / history row — explicit CREATE
        tables
            .create_row("transfers", transfer.id.as_str())
            .set("from_addr", &transfer.from)
            .set("to_addr", &transfer.to)
            .set("amount", &transfer.amount);

        // Mutable balance — explicit UPSERT (not update_row auto-CREATE).
        // PK = address (id in schema). Credit recipient with delta add (Postgres).
        // If your map already computed an *absolute* balance, use .set("amount", &balance) instead.
        tables
            .upsert_row("balances", transfer.to.as_str())
            .set("address", &transfer.to)
            .add("amount", transfer.amount.as_str());

        tables
            .upsert_row("balances", transfer.from.as_str())
            .set("address", &transfer.from)
            .sub("amount", transfer.amount.as_str());
    }

    Ok(tables.to_database_changes())
}
```

> The Rust path `pb::database::DatabaseChanges` is **deprecated**. Prefer  
> `substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges`.  
> The wire FQN `sf.substreams.sink.database.v1.DatabaseChanges` is unchanged.

> Full `Tables` API, composite PKs, and delta trait bounds → **`substreams-sql`**.

#### Step 4c — Wire `db_out` in the manifest (modules + imports + sink)

```yaml
imports:
  database: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v4.0.0/substreams-sink-database-changes-v4.0.0.spkg
  sql: https://github.com/streamingfast/substreams-sink-sql/releases/download/protodefs-v1.0.7/substreams-sink-sql-protodefs-v1.0.7.spkg

modules:
  - name: map_transfers
    kind: map
    initialBlock: 6082465
    inputs:
      - source: sf.ethereum.type.v2.Block
    output:
      type: proto:myproject.v1.Transfers

  - name: db_out
    kind: map
    initialBlock: 6082465
    inputs:
      - map: map_transfers
    output:
      type: proto:sf.substreams.sink.database.v1.DatabaseChanges

sink:
  module: db_out
  type: sf.substreams.sink.sql.service.v1.Service   # sf.substreams.sink.sql.v1.Service is DEPRECATED
  config:
    schema: ./schema.sql
    engine: postgres        # inert for the CLI (dialect comes from the DSN); matters for hosted deploys
```

Import the official database-changes (+ sql protodefs) spkg — do not define the proto yourself. The Rust crate emits changes at runtime; the **spkg import** is how the package resolves the type surface for packing and sink config.

#### Step 4d — Deploy to Postgres or ClickHouse

Load **`substreams-sql`** for module design and **`substreams-sink-deploy-local`** for the self-managed ops path (or **`substreams-hosted-sink`** for StreamingFast-hosted). Short recipe for Database Changes:

```bash
export DSN="psql://user:pass@localhost:5432/mydb?sslmode=disable"   # not postgresql://; no double-scheme

substreams build
# setup applies schema.sql + system tables (cursors, …) from the sink: block
substreams-sink-sql setup "$DSN" ./substreams.yaml

# run <dsn> <manifest|spkg> [range] — module comes from sink: only (not a trailing CLI arg)
substreams-sink-sql run "$DSN" ./substreams.yaml "6082465:+1000" \
  --on-module-hash-mismatch=warn \
  --batch-block-flush-interval=1   # short smoke ranges; default flush is 1000 blocks
```

For ClickHouse, prefer **from-proto** (insert-only) unless you accept CDC insert-only + no DB reorg management — see `substreams-sql` / `substreams-sink-deploy-local`.

### 5. Output: graph_out (optional / legacy)

> **Greenfield conversions should use the SQL sink (`db_out`) in Section 4.**  
> `graph_out` / `EntityChanges` is for pipelines that still need Graph Node, hosted subgraphs, or `substreams-sink-subgraph` — not the default migration path.
>
> **Crate caveat:** do **not** add `substreams-entity-change` on modern `substreams = "0.7"` — it pins `prost 0.11` / `substreams 0.5` and type-conflicts with the rest of the stack. Prefer **inlining** the canonical `EntityChanges` proto (see **`substreams-sink`**).
>
> Wire compatibility and a working `graph_out` sketch live in the `substreams-sink` skill. This convert skill does not re-teach full entity-change tutorials.


### 6. Dynamic Data Sources (Templates)

Subgraphs use templates for contracts deployed at runtime (e.g., Uniswap pair contracts). In Substreams, use a factory pattern with a store:

```yaml
# substreams.yaml
modules:
  - name: map_new_pairs          # detects factory events → emits pair addresses
    kind: map
    inputs:
      - source: sf.ethereum.type.v2.Block
    output:
      type: proto:myproject.v1.PairAddresses

  - name: store_pairs             # caches known pair addresses
    kind: store
    updatePolicy: set_if_not_exists
    valueType: string
    inputs:
      - map: map_new_pairs

  - name: map_pair_events         # processes events only for known pairs
    kind: map
    inputs:
      - source: sf.ethereum.type.v2.Block
      - store: store_pairs
        mode: get
    output:
      type: proto:myproject.v1.PairEvents
```

## Cargo.toml for Subgraph Conversion

> **Version note**: Verify the latest compatible versions at [crates.io](https://crates.io). The versions below are the current recommended set at the time of writing; check `substreams`, `substreams-ethereum`, and `substreams-database-change` for updates.

```toml
[package]
name    = "my_substreams"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
substreams                = "0.7"
substreams-ethereum       = "0.11"
substreams-database-change = "4"   # for db_out / SQL sink
prost                     = "0.13"
prost-types               = "0.13"
hex                       = "0.4"
hex-literal               = "0.4"
num-bigint                = "0.4"
ethabi                    = "17"

[build-dependencies]
substreams-ethereum = "0.11"

[profile.release]
lto       = true
opt-level = "s"
strip     = "debuginfo"
```

## Complete Manifest Example

```yaml
specVersion: v0.1.0
package:
  name: erc20_subgraph_conversion
  version: v0.1.0
  url: https://github.com/myorg/erc20_subgraph_conversion   # set it — avoids package.url warning
  description: ERC20 subgraph converted to Substreams         # set it — avoids package.description warning
network: mainnet

imports:
  database: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v4.0.0/substreams-sink-database-changes-v4.0.0.spkg
  sql: https://github.com/streamingfast/substreams-sink-sql/releases/download/protodefs-v1.0.7/substreams-sink-sql-protodefs-v1.0.7.spkg
  # Optional foundational filter package (full URL, not name@version):
  # eth_common: https://spkg.io/v1/packages/ethereum-common/v0.3.3

protobuf:
  files:
    - transfers.proto
  importPaths:
    - ./proto
  excludePaths: [sf/substreams, google]

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/erc20_subgraph_conversion.wasm

modules:
  - name: map_transfers
    kind: map
    initialBlock: 6082465
    inputs:
      - source: sf.ethereum.type.v2.Block
    output:
      type: proto:myproject.v1.Transfers

  - name: db_out
    kind: map
    initialBlock: 6082465
    inputs:
      - map: map_transfers
    output:
      type: proto:sf.substreams.sink.database.v1.DatabaseChanges

sink:
  module: db_out
  type: sf.substreams.sink.sql.service.v1.Service
  config:
    schema: ./schema.sql
    engine: postgres
```

## Common Pitfalls

| Subgraph Pitfall | Substreams Solution |
|---|---|
| `startBlock: 0` on all data sources | Use the actual contract deployment block as `initialBlock` |
| Entities auto-saved to store | Persist with `db_out` + SQL; use `store` only for intermediate WASM state |
| Mutable entity balances on ClickHouse | Use **PostgreSQL + Database Changes** (CH/from-proto are insert-only) |
| `event.params.value.toBigDecimal()` | `event.value.to_decimal(decimals)` (substreams BigInt helper) |
| `Address.fromString(hex)` | `hex::decode(addr.trim_start_matches("0x"))` or `Hex` + `0x` prefix |
| Template / dynamic data source | Factory pattern: store of addresses + filter in map module |
| `BigDecimal.plus()` across blocks | `store` with `updatePolicy: add` and `valueType: bigdecimal`, or Postgres delta `.add` |
| `entity.save()` on every event | Explicit `create_row` / `update_row` / `upsert_row` in `db_out` (SQL-first). Do **not** route new migrations through `graph_out` |
| Assuming `update_row` auto-creates | It does **not** — use `upsert_row` or `create_row` when the row may not exist |
| `ethereum_common@v…` import 404 | Use full URL `https://spkg.io/v1/packages/ethereum-common/v0.3.3` |
| Module as trailing sink-sql CLI arg | Module comes from `sink:` only; CLI is `setup`/`run <dsn> <manifest\|spkg> [range]` |
| Subgraph grafting (resume from snapshot) | No equivalent in Substreams — `initialBlock` is the only start point. Set it to the contract deployment block; there is no way to resume from a prior subgraph deployment's state. Plan for a full backfill from `initialBlock`. |

## Testing

```bash
# Build
substreams build

# Interactive visual debugger — best tool for inspecting module outputs during conversion
substreams gui ./substreams.yaml map_transfers -s 6082465 -t +100

# Verify map output looks correct
substreams run ./substreams.yaml map_transfers \
  -s 6082465 -t +100 \
  -o json

# Verify database change output
substreams run ./substreams.yaml db_out \
  -s 6082465 -t +1000 \
  -o jsonl
```

## References

- [Substreams Documentation](https://substreams.streamingfast.io)
- [substreams-database-change crate](https://github.com/streamingfast/substreams-sink-database-changes)
- [substreams-sink-sql (Postgres / ClickHouse)](https://github.com/streamingfast/substreams-sink-sql)
