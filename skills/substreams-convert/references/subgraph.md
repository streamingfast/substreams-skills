# Subgraph → Substreams Conversion Guide

Load this file when converting a subgraph (The Graph) into a Substreams pipeline.

## Conceptual Mapping

| Subgraph Concept | Substreams Equivalent |
|---|---|
| `schema.graphql` entity | Protobuf message (`.proto`) |
| `subgraph.yaml` data source | `substreams.yaml` module + `initialBlock` |
| AssemblyScript event handler | Rust `map` module |
| Entity store (auto) | Explicit `store` module |
| `graph-node` indexer | `substreams-sink-subgraph` or direct `graph_out` |
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
>   eth_common: ethereum_common@v0.3.3
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
> `ethereum_common@v0.3.3` also provides `filtered_calls`, `filtered_transactions`, and
> `filtered_events_and_calls`. For the full block-filtering guide including the SQE query
> syntax, rolling a custom `blockIndex`, and Solana's `solana_common` equivalents, see
> the `substreams-dev` skill's `references/block-filtering.md`.
>
> The in-handler address check shown below is a fallback for when no foundational
> `filtered_*` module covers your use case. When depending on `eth_common:filtered_events`,
> you do **not** need an in-handler address check — the module has already filtered for you.

```rust
use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;
use substreams_ethereum::Event;

use crate::abi::erc20::events::Transfer as TransferEvent;
use crate::pb::myproject::v1::{Transfer, Transfers};

// Contract address (from subgraph.yaml source.address)
const TOKEN_ADDRESS: [u8; 20] =
    hex_literal::hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

#[substreams::handlers::map]
pub fn map_transfers(block: Block) -> Result<Transfers, Error> {
    let mut transfers = Transfers::default();

    for log in block.logs() {
        // Filter by contract address (replaces subgraph data source address filter).
        // In production, prefer a foundational filtered_events module as input
        // so blocks with no matching logs are skipped entirely (see note above).
        if log.address() != TOKEN_ADDRESS {
            continue;
        }

        // Decode event (replaces AssemblyScript event handler binding)
        if let Some(event) = TransferEvent::match_and_decode(log) {
            let tx_hash = format!("0x{}", hex::encode(log.receipt.transaction.hash.as_slice()));
            let id = format!("{}-{}", tx_hash, log.index());

            transfers.transfers.push(Transfer {
                id,
                from: format!("0x{}", hex::encode(event.from)),
                to: format!("0x{}", hex::encode(event.to)),
                amount: event.value.to_decimal(6).to_string(), // 6 = USDC decimals; parameterize per token — never hardcode 18
                block_number: block.number,
                timestamp: block.timestamp_seconds(),
            });
        }
    }

    Ok(transfers)
}
```

> **`substreams_ethereum::Event`** provides `match_and_decode` — equivalent to subgraph's automatic event binding. Add `substreams-ethereum = "0.11"` to `Cargo.toml`.

> **Multiple contract addresses**: Subgraphs commonly index many contracts (e.g. all pairs in a factory). In Substreams, use a `HashSet` of known addresses populated from a store, then check `known_addresses.contains(log.address())`. For dynamically discovered addresses (factory pattern), see Section 6 (Dynamic Data Sources).

```rust
// For a fixed allow-list of contracts:
const KNOWN_TOKENS: &[[u8; 20]] = &[
    hex_literal::hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), // USDC
    hex_literal::hex!("dAC17F958D2ee523a2206206994597C13D831ec7"), // USDT
];

// In handler:
if !KNOWN_TOKENS.contains(&log.address()) {
    continue;
}
```

### 4. Migrating Entity Storage — SQL Sink (Postgres / ClickHouse)

The recommended way to persist subgraph entity data in Substreams is the **SQL sink** (Postgres or ClickHouse), using the `db_out` module pattern with `DatabaseChanges`. This replaces subgraph's auto-managed entity store and gives you a full relational database with SQL Delta support — ideal for subgraph migrations.

> **`store` modules** are still used in Substreams, but for a specific purpose: caching intermediate state that other modules need to read during processing (e.g. tracking dynamically discovered contract addresses — see Section 6). They are not the replacement for subgraph entity persistence. Use `db_out` + SQL sink for that.

#### Step 4a — Define the SQL schema

Create a `schema.sql` that mirrors your subgraph's `schema.graphql` entities:

**Before (GraphQL SDL):**
```graphql
type Balance @entity {
  id: ID!
  address: String!
  amount: BigDecimal!
}
```

**After (`schema.sql`):**
```sql
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

Add a `db_out` map module that converts your map output into database change records:

```rust
use substreams::store::{DeltaBigDecimal, Deltas};
use substreams_database_change::pb::database::{DatabaseChanges, TableChange};
use substreams_database_change::tables::Tables;

#[substreams::handlers::map]
pub fn db_out(
    transfers: Transfers,
) -> Result<DatabaseChanges, substreams::errors::Error> {
    let mut tables = Tables::new();

    for transfer in transfers.transfers {
        // Upsert a balance row — SQL Delta handles CREATE vs UPDATE automatically
        tables
            .update_row("balances", &transfer.to)
            .set("address", &transfer.to)
            .set_bigdecimal("amount", &transfer.amount.parse().unwrap_or_default());
    }

    Ok(tables.to_database_changes())
}
```

#### Step 4c — Wire `db_out` in the manifest

```yaml
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
```

#### Step 4d — Deploy to Postgres or ClickHouse

Load the **`substreams-sink-deploy` skill** for the full sink deployment workflow. The short version:

```bash
# Apply schema
psql "$DATABASE_URL" -f schema.sql

# Run the sink
substreams-sink-sql run \
  "psql://$DATABASE_URL" \
  ./substreams.yaml \
  db_out \
  --on-module-hash-mistmatch=warn
```

For ClickHouse, the schema uses `ReplacingMergeTree` instead of plain `PRIMARY KEY` — see the `substreams-sink-deploy` skill for details.

> **SQL Delta**: `substreams-database-change` v4 includes SQL Delta support. `tables.update_row()` automatically emits the correct `CREATE` / `UPDATE` / `DELETE` operation based on whether the row already exists, mirroring subgraph's `entity.save()` semantics without requiring explicit `store` modules for persistence.


### 5. Output: graph_out ~~(deprecated)~~

> **Graph Node no longer supports Substreams-powered subgraphs.** The `graph_out` / `EntityChanges` output pattern is deprecated and should not be used for new projects. Use the SQL sink (`db_out` + Postgres or ClickHouse) described in Section 4 instead.
>
> This section is retained for reference only, in case you are maintaining an existing `graph_out` module.


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

protobuf:
  files:
    - transfers.proto
  importPaths:
    - ./proto

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
```

## Common Pitfalls

| Subgraph Pitfall | Substreams Solution |
|---|---|
| `startBlock: 0` on all data sources | Use the actual contract deployment block as `initialBlock` |
| Entities auto-saved to store | Use explicit `store` module with appropriate `updatePolicy` |
| `event.params.value.toBigDecimal()` | `event.value.to_decimal(decimals)` (substreams BigInt helper) |
| `Address.fromString(hex)` | `hex::decode(addr.trim_start_matches("0x"))` |
| Template / dynamic data source | Factory pattern: store of addresses + filter in map module |
| `BigDecimal.plus()` across blocks | `store` with `updatePolicy: add` and `valueType: bigdecimal` |
| `entity.save()` on every event | Emit from map → drive store deltas → `graph_out` reads deltas |
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
