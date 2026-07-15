---
name: substreams-sql
description: Expert knowledge for building SQL database sinks from Substreams. Covers sink-type selection, PostgreSQL vs ClickHouse, Database Changes vs From-proto modes, relational mappings, and materialized views.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.3.0
  author: StreamingFast
  documentation: https://substreams.streamingfast.io
---

# Substreams SQL Expert

Expert assistant for building SQL database sinks from Substreams data - transforming blockchain data into relational databases.

## Prerequisites

### Installing substreams-sink-sql

The `substreams-sink-sql` CLI tool is required for database sink workflows.

**Homebrew (macOS/Linux):**
```bash
brew install streamingfast/tap/substreams-sink-sql
```

**Binary Release:**
Download from [GitHub Releases](https://github.com/streamingfast/substreams-sink-sql/releases)

**Docker:**
```bash
docker pull ghcr.io/streamingfast/substreams-sink-sql:latest
# Or specific version
docker pull ghcr.io/streamingfast/substreams-sink-sql:v4.12.0
```

> **Note:** Do NOT use `substreams-sink-postgres` - this is an old deprecated name. The current tool is `substreams-sink-sql` which supports both PostgreSQL and ClickHouse.

## Core Concepts

### Pre-flight: Mandatory Choice Tree (ALWAYS run first)

**Do not write manifests, schemas, or Rust until the user has chosen (or already stated) each applicable step below.** Never assume SQL, never assume Postgres, never assume Database Changes.

#### How to ask (MANDATORY)

1. **One question per turn** — complete Step 1 before Step 2, etc. Never stack the whole tree in one message.
2. **Offer explicit choices** every time.
3. **Always include** a final **“Other / enter a custom answer”** option (even when the skill only supports a fixed set — use it to catch misroutes and explain limits).
4. Skip steps already answered by the user.

#### Step 1 — Sink type (own turn)

Always offer these products (even if they said “SQL” casually), plus custom:

| Choice | Product | Destination | When to pick |
|---|---|---|---|
| **SQL** | [`substreams-sink-sql`](https://github.com/streamingfast/substreams-sink-sql) | PostgreSQL or ClickHouse | Analytical queries, joins, BI |
| **KV** | [`substreams-sink-kv`](https://github.com/streamingfast/substreams-sink-kv) | Key-value stores | Simple key lookups |
| **Files** | [`substreams-sink-files`](https://github.com/streamingfast/substreams-sink-files) | Local FS / S3 / GCS | Data lakes, batch ETL |
| **Other / enter a custom answer** | — | Free-text | Re-route or clarify |

- If **KV** or **Files** → hand off to `substreams-sink` / `substreams-sink-deploy-local`. Stop SQL guidance.
- Only continue when the user picks **SQL** (`substreams-sink-sql`).

**Who runs the SQL sink?** Separate turn when destination is SQL (or when “SQL” was vague):

| Choice | Skill after the module is built |
|---|---|
| **Self-managed** — user runs `substreams-sink-sql` | `substreams-sink-deploy-local` |
| **StreamingFast hosted** — Portal deploys/operates (Postgres or ClickHouse only) | **`substreams-hosted-sink`** |
| **Other / enter a custom answer** | Clarify |

Do not fold “hosted” into a single vague “SQL” option. Module design (this skill) is shared; ops path is not.

#### Step 2 — Database engine (SQL only; own turn)

| Choice | Best for | Notes |
|---|---|---|
| **PostgreSQL** | Operational apps, reorg-safe UPSERTs, ACID, delta updates | Both sink modes (Step 3) |
| **ClickHouse** | High-volume analytics, time-series, OLAP | **From proto definition only** |
| **Other / enter a custom answer** | — | If not Postgres/ClickHouse, explain hard limit and re-offer |

Do not invent other engines for this skill. Hosted SQL also supports only PostgreSQL and ClickHouse (`substreams-hosted-sink`).

#### Step 3 — Mapping mode (own turn when applicable)

| Engine | Modes to offer | Rule |
|---|---|---|
| **PostgreSQL** | **Database Changes** *and* **From proto definition** *and* **Other / custom** | Always present both modes + custom; let the user choose |
| **ClickHouse** | **From proto definition only** | **Do not offer Database Changes.** Explain briefly, then proceed (no mode choice). |

**Mode summary:**

| Mode | Output module | CLI path | Ops supported | Best for |
|---|---|---|---|---|
| **Database Changes** | `proto:sf.substreams.sink.database.v1.DatabaseChanges` | `setup` + `run` | INSERT, UPDATE, UPSERT, DELETE, delta ops | Mutable state (**PostgreSQL only**) |
| **From proto definition** | Your own proto + `schema.*` annotations | `from-proto` | Insert-only | Proto→table, analytics, **required for ClickHouse** |

#### Step 4 — Remaining project inputs (one question per turn)

After Steps 1–3, if any of these are still missing, ask **one at a time** with choices + **Other / custom**:

| Required input | Why it matters |
|---|---|
| **Chain + contract/protocol** | Module inputs and event shape |
| **Data shape / tables** | Entities, fields, primary keys |
| **Block range** | `initialBlock` and test window |
| **Aggregations needed?** | Delta updates (Postgres + Database Changes only) vs raw rows vs MVs |

### What is Substreams SQL?

Substreams SQL enables you to:
- **Transform blockchain data** into structured SQL tables
- **Stream** into **PostgreSQL** or **ClickHouse** via `substreams-sink-sql`
- **Choose a mapping mode**: Database Changes (Postgres only) or From proto definition (Postgres + ClickHouse)
- **Build materialized views** with real-time updates
- **Handle reorgs** with cursor-based streaming

### Two Mapping Modes (SQL only)

1. **Database Changes** — module emits `DatabaseChanges` (CDC). **PostgreSQL only.**
2. **From proto definition** — module emits a custom proto; sink maps fields to tables via annotations. **PostgreSQL and ClickHouse** (required for ClickHouse).

## Database Changes (CDC) Approach

> **PostgreSQL only.** Do not use Database Changes with ClickHouse. If the user chose ClickHouse, skip this entire section and use [From proto definition](#from-proto-definition-approach) instead.

### Overview

The CDC approach streams individual database operations (INSERT, UPDATE, DELETE) to maintain real-time consistency on **PostgreSQL**.

### Key Components

**Protobuf Schema**: The `DatabaseChanges` protobuf type is provided by the official `substreams-sink-database-changes` spkg — you do NOT need to define your own proto. Import it in your manifest (see Manifest Configuration below).

**Rust Implementation**:
```rust
use substreams::prelude::*;
// Always use the fully-qualified v4 path:
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
// DEPRECATED (v3): substreams_database_change::pb::database::DatabaseChanges — still compiles
// on crate v4 but will be removed; use the FQN above.
use substreams_database_change::tables::Tables;

#[substreams::handlers::map]
pub fn db_out(events: Events) -> Result<DatabaseChanges, Error> {
    let mut tables = Tables::new();

    for transfer in events.transfers {
        tables
            .create_row("transfers", format!("{}-{}", transfer.tx_hash, transfer.log_index))
            .set("tx_hash", transfer.tx_hash)
            .set("from_addr", transfer.from)
            .set("to_addr", transfer.to)
            .set("amount", transfer.amount)
            .set("block_num", transfer.block_number)
            .set("timestamp", transfer.timestamp);
    }

    for balance in events.balance_changes {
        tables
            .update_row("balances", balance.address)
            .set("balance", balance.new_balance)
            .set("updated_at", balance.timestamp);
    }

    Ok(tables.to_database_changes())
}
```

### Manifest Configuration

The manifest requires importing the database changes and sink-sql protodefs spkgs, and a `sink:` section:

```yaml
specVersion: v0.1.0
package:
  name: my-substreams-sql
  version: 1.3.0
  url: https://github.com/myorg/my-substreams-sql   # set it — avoids package.url warning
  description: SQL sink substreams for <protocol>     # set it — avoids package.description warning

imports:
    # Use the latest v4+ spkg so the proto FQN matches the Rust crate:
    database: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v4.0.0/substreams-sink-database-changes-v4.0.0.spkg
    sql: https://github.com/streamingfast/substreams-sink-sql/releases/download/protodefs-v1.0.7/substreams-sink-sql-protodefs-v1.0.7.spkg
    # Note: v3 spkg import still works but exposes the deprecated short-path proto names.

protobuf:
  excludePaths:
    - sf/substreams
    - google

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/my_substreams_sql.wasm

network: mainnet

modules:
  - name: db_out
    kind: map
    inputs:
      - map: map_events
    output:
      type: proto:sf.substreams.sink.database.v1.DatabaseChanges

sink:
  module: db_out
  type: sf.substreams.sink.sql.v1.Service
  config:
    schema: ./schema.sql
    engine: postgres
```

**Cargo.toml dependency** (v4 with delta updates support):
```toml
[dependencies]
substreams-database-change = "4"  # Latest: 4.0.0
```

> **Note — graph-out (Entity Changes) is a DIFFERENT sink type:** if your project also needs The Graph output, use `sf.substreams.sink.entity.v1.EntityChanges` (NOT `DatabaseChanges`) and follow the inline-proto workaround in `substreams-sink` under "Graph Node / The Graph Output". Do not add `substreams-entity-change = "1"` directly — it conflicts with `prost = "0.13"`.

**Running the sink** (DSN is passed on the command line, not in the manifest):
```bash
# 1. Build
substreams build

# 2. Setup system tables + apply schema.sql
substreams-sink-sql setup "psql://user:pass@localhost:5432/db?sslmode=disable" my-substreams-sql-v0.1.0.spkg

# 3. Run the sink
substreams-sink-sql run "psql://user:pass@localhost:5432/db?sslmode=disable" my-substreams-sql-v0.1.0.spkg
```

### Advantages
- ✅ **Real-time consistency** - Changes applied immediately
- ✅ **Handles reorgs** - Can reverse/replay operations
- ✅ **Efficient updates** - Only changed data is transmitted
- ✅ **ACID compliance** - Database transactions ensure consistency

### Use Cases
- Real-time dashboards
- Trading applications
- Balance tracking
- Event sourcing

### Delta Updates for Aggregations

Delta updates enable atomic modifications to database rows without read-modify-write cycles. This is essential for building aggregation patterns like candles, counters, and real-time analytics — pushing chain-wide computation directly into the database instead of using Substreams store modules.

> **Note:** Delta updates require PostgreSQL and `substreams-sink-sql` >= v4.12.0.

```rust
use substreams_database_change::tables::Tables;
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;

#[substreams::handlers::map]
fn db_out(events: Events) -> Result<DatabaseChanges, substreams::errors::Error> {
    let mut tables = Tables::new();

    for event in &events.items {
        // Composite primary keys use an array of tuples
        tables.upsert_row("aggregates", [
            ("key1", value1),
            ("key2", value2),
        ])
            .set_if_null("first_seen", &timestamp)  // First write wins
            .set("last_seen", &timestamp)            // Always overwrite
            .max("highest", value)                   // Track maximum
            .min("lowest", value)                    // Track minimum
            .add("total", amount)                    // Accumulate
            .add("count", 1i64);                     // Count
    }

    Ok(tables.to_database_changes())
}
```

**Supported Delta Operations:**

| Operation | SQL Equivalent | Use Case |
|-----------|---------------|----------|
| `set_if_null` | `COALESCE(column, value)` | First-write-wins |
| `set` | `column = value` | Always overwrite |
| `max` | `GREATEST(column, value)` | Track maximum |
| `min` | `LEAST(column, value)` | Track minimum |
| `add` | `COALESCE(column, 0) + value` | Accumulate |
| `sub` | `COALESCE(column, 0) - value` | Decrement |

**Important Notes:**
- The `add()` operation requires values implementing `NumericAddable` — pass owned values (`volume.clone()` or `volume`) rather than `&String` references
- Ordinals are automatically managed by the `Tables` struct — no manual management needed

## From proto definition Approach

### Overview

**From proto definition** (also called relational mappings / `from-proto`) maps your **custom** output protobuf to SQL tables using `schema.*` annotations. The sink generates the schema and inserts rows from the proto shape — **insert-only** (no UPDATE/DELETE from the module).

- **PostgreSQL**: optional alternative to Database Changes
- **ClickHouse**: **required** mode (Database Changes is not supported)

CLI:
```bash
substreams-sink-sql from-proto "$DSN" ./substreams.yaml [output-module]
```

### Proto with SQL annotations

Import the sink schema protodef and annotate tables/fields:

```proto
syntax = "proto3";
package transfers;

import "google/protobuf/timestamp.proto";
import "sf/substreams/sink/sql/schema/v1/schema.proto";

message Output {
  repeated Transfer transfers = 1;
}

message Transfer {
  option (schema.table) = {
    name: "transfers"
    // Required for ClickHouse — omit clickhouse_table_options for Postgres-only
    clickhouse_table_options: {
      // MUST start with the primary_key fields in the same order (see hard rule below)
      order_by_fields: [{ name: "tx_hash" }, { name: "log_index" }]
      partition_fields: [{ name: "_block_timestamp_", function: toYYYYMM }]
    }
  };

  string tx_hash = 1 [(schema.field) = { primary_key: true }];
  uint32 log_index = 2 [(schema.field) = { primary_key: true }];
  string from_addr = 3;
  string to_addr = 4;
  string amount = 5 [(schema.field) = { convertTo: { uint256{} } }];
  string contract_addr = 6;
  google.protobuf.Timestamp timestamp = 7;
}
```

**ClickHouse:** every table message **must** set `clickhouse_table_options` with at least `order_by_fields`. Missing options fail with `clickhouse table options not set for table "..."`.

#### ClickHouse hard rule: primary key must prefix ORDER BY

The sink emits `ReplacingMergeTree` with:

- `PRIMARY KEY (...)` from fields marked `[(schema.field) = { primary_key: true }]` (declaration order of PK fields)
- `ORDER BY (...)` from `clickhouse_table_options.order_by_fields`

**ClickHouse requires the primary key to be a prefix of the sorting key.** If `ORDER BY` is `(slot, id)` then `PRIMARY KEY` must be `(slot)` or `(slot, id)` — **not** `(id)` alone.

| ❌ Invalid (common agent mistake) | ✅ Valid |
|---|---|
| PK `(id)` + `order_by: [slot, id]` | PK `(id)` + `order_by: [id]` or `[id, slot]` |
| PK `(id)` + `order_by: [slot, signature, id]` | PK `(slot, id)` + `order_by: [slot, id, …]` |
| PK `(tx_hash)` + `order_by: [contract, tx_hash]` | PK `(tx_hash, log_index)` + `order_by: [tx_hash, log_index]` |

**Error you will see if violated:**

```text
DB::Exception: Primary key must be a prefix of the sorting key, but the column
in the position 0 is slot, not id
```

**How to fix (pick one):**

1. **Identity-first (simplest):** only mark the unique row key(s) as PK and put those **first** in `order_by_fields`. Extra sort columns may follow the PK prefix.
2. **Time/slot-first analytics:** mark **every leading ORDER BY column that is part of uniqueness** as `primary_key: true` in the **same order** as `order_by_fields` (e.g. `slot` + `id` both PK if `ORDER BY (slot, id)`).

```proto
// Solana swap row — identity-first (safe default)
message Swap {
  option (schema.table) = {
    name: "raydium_clmm_swaps"
    clickhouse_table_options: {
      order_by_fields: [
        { name: "id" },
        { name: "slot" }   // optional secondary sort AFTER pk prefix
      ]
      // Prefer low-cardinality partitions (month). Do NOT partition by raw slot.
      partition_fields: [{ name: "_block_timestamp_", function: toYYYYMM }]
    }
  };
  string id = 1 [(schema.field) = { primary_key: true }]; // e.g. signature-ordinal
  uint64 slot = 2;
  // ...
}

// Same table — slot-first (only if you need range scans by slot)
message SwapBySlot {
  option (schema.table) = {
    name: "raydium_clmm_swaps"
    clickhouse_table_options: {
      order_by_fields: [{ name: "slot" }, { name: "id" }]
      partition_fields: [{ name: "_block_timestamp_", function: toYYYYMM }]
    }
  };
  uint64 slot = 1 [(schema.field) = { primary_key: true }];
  string id = 2 [(schema.field) = { primary_key: true }];
  // ...
}
```

**Checklist before deploy (ClickHouse / hosted SQL):**

1. List fields with `primary_key: true` in order → that is `PRIMARY KEY`.
2. List `order_by_fields` names in order → that is `ORDER BY`.
3. Confirm PK is exactly the first N columns of ORDER BY (N ≥ 1).
4. `partition_fields` should use coarse keys (`toYYYYMM(_block_timestamp_)`), not high-cardinality columns like raw `slot` alone (creates too many partitions).
5. **No reserved ClickHouse identifiers as column names** — see [ClickHouse reserved column names](#clickhouse-reserved-column-names) below.

#### ClickHouse reserved column names

The sink generates bare SQL identifiers from proto field names. **ClickHouse reserves many words**; using them as columns fails `CREATE TABLE` with `SYNTAX_ERROR` (often near `,` after the reserved token).

| ❌ Common IDL / proto name | ✅ Rename to |
|---|---|
| `index` | `config_index` |
| `keys` | `account_keys` |
| `value` | `config_value` |
| `status` | `status_code` |
| `order`, `group`, `table`, `database`, `default`, `system`, `format`, `settings`, `primary`, `engine`, `select`, `from`, `where`, `limit`, `offset`, `with`, `as`, `interval`, `timestamp`, `date`, `null` | descriptive alias (e.g. `pool_status`, `order_value`) |

**Always** scan every `schema.table` field (including flattened nested struct fields from IDLs) before first ClickHouse deploy. Prefer renaming in **proto** (and Rust) rather than quoting — sink DDL may not quote identifiers.

After a rename, existing tables will **not** gain new columns (`CREATE TABLE IF NOT EXISTS`) — drop/recreate or hosted `ResetDeployment` with `drop_schema: true` (see [Schema evolution](#schema-evolution-from-proto)).

### Manifest (from-proto)

**Required:** load SQL schema annotations via **buf descriptor set** (do not only vendor a half path of `schema.proto` without deps). Canonical pattern from the ClickHouse showcase:

```yaml
specVersion: v0.1.0
package:
  name: my-substreams-sql
  version: v0.1.0

network: solana   # or mainnet / ethereum-mainnet, etc.

protobuf:
  files:
    - events.proto
  descriptorSets:
    - module: buf.build/streamingfast/substreams-sink-sql
  importPaths:
    - ./proto
  excludePaths:
    - sf/substreams
    - google

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/my_substreams_sql.wasm

modules:
  - name: map_transfer
    kind: map
    inputs:
      - source: sf.ethereum.type.v2.Block   # or sf.solana.type.v1.Block
    output:
      type: proto:transfers.Output   # your proto — NOT DatabaseChanges

sink:
  module: map_transfer
  type: sf.substreams.sink.sql.v1.Service
  config: {}   # schema is derived from proto annotations
```

Without `descriptorSets`, `substreams build` often fails resolving `sf/substreams/sink/sql/schema/v1/schema.proto`.

### Run

```bash
# PostgreSQL
substreams-sink-sql from-proto "psql://user:pass@localhost:5432/db?sslmode=disable" ./substreams.yaml

# ClickHouse (Native TCP port 9000 / 9440 — not HTTP 8123)
substreams-sink-sql from-proto "clickhouse://default:@localhost:9000/default" ./substreams.yaml
```

### Advantages
- ✅ Less hand-written CDC code — tables inferred from proto
- ✅ Works on **both** PostgreSQL and ClickHouse
- ✅ Foreign keys / relational annotations supported
- ❌ Insert-only — no UPDATE/UPSERT/DELETE from the module
- ❌ Not a substitute for Database Changes when you need mutable state on Postgres

### Advantages of Database Changes (Postgres) vs From proto
Use Database Changes when you need UPSERT, deletes, or delta aggregations. Use From proto for simpler insert mirrors and for **all ClickHouse** sinks.

## PostgreSQL Implementation

### Setup and Configuration

**Docker Setup**:
```yaml
# docker-compose.yml
version: '3.8'
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: substreams
      POSTGRES_USER: substreams
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./schema.sql:/docker-entrypoint-initdb.d/schema.sql

volumes:
  postgres_data:
```

Set `POSTGRES_PASSWORD` in a `.env` file (never commit it) or via your shell before running `docker compose up`.

**Running the Sink** (mode-dependent):
```bash
# The DSN is passed as a CLI argument, not in the manifest
# For local development (sslmode=disable is only safe on localhost):

# Database Changes (setup + run)
substreams-sink-sql setup "psql://substreams:${POSTGRES_PASSWORD}@localhost:5432/substreams?sslmode=disable" ./my-substreams.spkg
substreams-sink-sql run "psql://substreams:${POSTGRES_PASSWORD}@localhost:5432/substreams?sslmode=disable" ./my-substreams.spkg

# Development mode (allows re-processing):
substreams-sink-sql run --development-mode "psql://substreams:${POSTGRES_PASSWORD}@localhost:5432/substreams?sslmode=disable" ./my-substreams.spkg

# From proto definition (alternative mode on Postgres)
substreams-sink-sql from-proto "psql://substreams:${POSTGRES_PASSWORD}@localhost:5432/substreams?sslmode=disable" ./substreams.yaml
```

### PostgreSQL-Specific Features

**JSONB Support**:
```sql
-- Store complex data as JSONB
CREATE TABLE transaction_logs (
    tx_hash VARCHAR(66),
    log_index INTEGER,
    data JSONB,
    topics JSONB
);

-- Index JSONB fields
CREATE INDEX idx_logs_data ON transaction_logs USING GIN (data);
```

**Materialized Views**:
```sql
-- Daily transfer volumes
CREATE MATERIALIZED VIEW daily_transfer_volumes AS
SELECT
    DATE(to_timestamp(timestamp)) as date,
    contract_address,
    COUNT(*) as transfer_count,
    SUM(amount) as total_volume
FROM erc20_transfers t
JOIN transactions tx ON t.tx_hash = tx.hash
GROUP BY DATE(to_timestamp(timestamp)), contract_address;

-- Refresh strategy (handled by Substreams)
CREATE UNIQUE INDEX ON daily_transfer_volumes (date, contract_address);
```

**Performance Tuning**:
```sql
-- Partitioning by block number
CREATE TABLE erc20_transfers (
    -- columns...
    block_number BIGINT
) PARTITION BY RANGE (block_number);

CREATE TABLE erc20_transfers_y2024 PARTITION OF erc20_transfers
    FOR VALUES FROM (18000000) TO (20000000);

-- Parallel processing
SET max_parallel_workers_per_gather = 4;
SET max_parallel_workers = 8;
```

### Best Practices for PostgreSQL

1. **Use appropriate data types**:
   ```sql
   -- Use NUMERIC for big integers (token amounts)
   amount NUMERIC(78,0)  -- Not BIGINT

   -- Use proper VARCHAR sizes
   address VARCHAR(42)   -- Ethereum addresses
   hash VARCHAR(66)      -- Transaction hashes
   ```

2. **Index strategically**:
   ```sql
   -- Query-specific indexes
   CREATE INDEX idx_transfers_token_date ON erc20_transfers(contract_address, block_number);

   -- Partial indexes for active data
   CREATE INDEX idx_active_balances ON token_balances(address) WHERE balance > 0;
   ```

3. **Use constraints**:
   ```sql
   -- Data validation
   ALTER TABLE erc20_transfers ADD CONSTRAINT check_positive_amount
       CHECK (amount >= 0);

   -- Foreign keys for referential integrity
   ALTER TABLE erc20_transfers ADD CONSTRAINT fk_transaction
       FOREIGN KEY (tx_hash) REFERENCES transactions(hash);
   ```

## ClickHouse Implementation

> **Mode lock:** ClickHouse supports **From proto definition only**. Never generate a `db_out` / `DatabaseChanges` module for ClickHouse, never run `setup`+`run` CDC against ClickHouse for this skill's guidance, and never offer Database Changes as a choice when the engine is ClickHouse.

### Setup and Configuration

**Docker Setup**:
```yaml
# docker-compose.yml
version: '3.8'
services:
  clickhouse:
    image: clickhouse/clickhouse-server:latest
    ports:
      - "9000:9000"   # Native TCP — required by substreams-sink-sql
      - "8123:8123"   # HTTP — not used by the sink
    environment:
      CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT: 1
    volumes:
      - clickhouse_data:/var/lib/clickhouse

volumes:
  clickhouse_data:
```

**Manifest Configuration** (ClickHouse — from-proto): use the full [from-proto manifest](#manifest-from-proto) with `descriptorSets` + your custom output proto (not DatabaseChanges).

**Running the Sink** (from-proto; DSN on the command line):
```bash
# Native protocol only (9000 / 9440). HTTP ports are rejected.
# ClickHouse Cloud: add ?secure=true and use port 9440
substreams-sink-sql from-proto "clickhouse://default:@localhost:9000/default" ./substreams.yaml
```

### ClickHouse Schema Design (via proto annotations)

Schema is generated from protobuf `schema.table` / `schema.field` annotations — not from a hand-written CDC `schema.sql` for Database Changes. Configure ORDER BY / PARTITION via `clickhouse_table_options` (required).

**Critical:** `primary_key: true` fields **must** be a **prefix** of `order_by_fields` (same names, same leading order). See [ClickHouse hard rule](#clickhouse-hard-rule-primary-key-must-prefix-order-by) above. Hosted sinks hit the same DDL path — a bad proto fails at `CREATE TABLE` with `Primary key must be a prefix of the sorting key`.

**Example analytics-oriented table options** (PK prefix matches ORDER BY):
```proto
message Erc20Transfer {
  option (schema.table) = {
    name: "erc20_transfers"
    clickhouse_table_options: {
      // Leading columns of ORDER BY must match primary_key fields
      order_by_fields: [
        { name: "contract_address" },
        { name: "tx_hash" },
        { name: "log_index" },
        { name: "_block_timestamp_" }
      ]
      partition_fields: [
        { name: "_block_timestamp_", function: toYYYYMM }
      ]
    }
  };
  string contract_address = 1 [(schema.field) = { primary_key: true }];
  string tx_hash = 2 [(schema.field) = { primary_key: true }];
  uint32 log_index = 3 [(schema.field) = { primary_key: true }];
  // non-PK columns may still appear later in order_by_fields
}
```

**Materialized views** (created in ClickHouse after the sink has created base tables):
```sql
CREATE MATERIALIZED VIEW hourly_transfer_stats
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(hour)
ORDER BY (contract_address, hour)
AS SELECT
    contract_address,
    toStartOfHour(_block_timestamp_) AS hour,
    count() AS transfer_count,
    sum(amount) AS total_volume
FROM erc20_transfers
WHERE _deleted_ = 0
GROUP BY contract_address, hour;
```

The sink uses `ReplacingMergeTree` with `_version_` / `_deleted_` for reorg-aware inserts. Prefer filtering `_deleted_ = 0` (or additive `if(_deleted_, -x, x)` patterns) in queries and MVs.

**ClickHouse-Specific Features**:
```sql
-- Compression and optimization
ALTER TABLE erc20_transfers MODIFY COLUMN amount Codec(Delta, ZSTD);

-- TTL for data lifecycle management
ALTER TABLE erc20_transfers MODIFY TTL toDate(_block_timestamp_) + INTERVAL 2 YEAR;

-- Dictionaries for dimension data
CREATE DICTIONARY token_metadata (
    address String,
    symbol String,
    decimals UInt8,
    name String
) PRIMARY KEY address
SOURCE(POSTGRESQL(
    host 'postgres'
    port 5432
    user 'substreams'
    password '<your-db-password>'
    db 'substreams'
    table 'token_metadata'
))
LIFETIME(300)
LAYOUT(HASHED());
```

### Performance Optimization

**Partition pruning** (depends on your partition key — default is often month of `_block_timestamp_`):
```sql
SELECT * FROM erc20_transfers
WHERE toYYYYMM(_block_timestamp_) = 202401
  AND _deleted_ = 0;
```

**Query Optimization**:
```sql
-- Projections / pre-aggregated tables on top of from-proto base tables
ALTER TABLE erc20_transfers ADD PROJECTION daily_stats (
    SELECT
        toDate(_block_timestamp_) AS date,
        contract_address,
        sum(amount),
        count()
    GROUP BY date, contract_address
);
```

## Materialized Views and Real-time Updates

### Concept

Materialized views provide pre-computed query results that update automatically as new data arrives.

### Implementation Patterns

**PostgreSQL Materialized Views**:
```sql
-- Token holder counts
CREATE MATERIALIZED VIEW token_holder_counts AS
SELECT
    token_address,
    COUNT(DISTINCT address) as holder_count,
    SUM(balance) as total_supply
FROM token_balances
WHERE balance > 0
GROUP BY token_address;

-- Refresh trigger (automated by Substreams)
CREATE OR REPLACE FUNCTION refresh_token_holder_counts()
RETURNS TRIGGER AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY token_holder_counts;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER token_balance_change_trigger
    AFTER INSERT OR UPDATE OR DELETE ON token_balances
    FOR EACH STATEMENT
    EXECUTE FUNCTION refresh_token_holder_counts();
```

**ClickHouse Materialized Views**:
```sql
-- Real-time price calculations
CREATE MATERIALIZED VIEW token_prices_mv
ENGINE = ReplacingMergeTree(timestamp)
ORDER BY (token_address, timestamp)
AS SELECT
    token_address,
    timestamp,
    price_usd,
    volume_24h,
    market_cap
FROM (
    -- Complex price calculation logic
    SELECT
        t.contract_address as token_address,
        toStartOfMinute(t.timestamp) as timestamp,
        calculatePrice(t.amount, p.eth_price) as price_usd,
        sum(t.amount) OVER (
            PARTITION BY t.contract_address
            ORDER BY t.timestamp
            RANGE BETWEEN INTERVAL 24 HOUR PRECEDING AND CURRENT ROW
        ) as volume_24h
    FROM erc20_transfers t
    JOIN eth_prices p ON toStartOfMinute(t.timestamp) = p.timestamp
    WHERE t.contract_address = '0x...' -- USDC
);
```

### Update Strategies

**Incremental Updates**:
```rust
// Track last processed block for incremental updates
#[substreams::handlers::store]
pub fn store_last_block(block: Block, store: StoreSetInt64) {
    store.set(0, "last_processed_block", &(block.number as i64));
}

// Only process new data
#[substreams::handlers::map]
pub fn incremental_db_out(
    block: Block,
    last_block_store: StoreGetInt64
) -> Result<DatabaseChanges, Error> {
    let last_processed = last_block_store.get_last("last_processed_block")
        .unwrap_or(0);

    if block.number <= last_processed as u64 {
        return Ok(DatabaseChanges::default()); // Skip already processed
    }

    // Process only new block data
    process_block_data(block)
}
```

**Cursor-based Consistency**:

Cursor management is handled automatically by `substreams-sink-sql`. On PostgreSQL Database Changes, run `setup` then `run` (cursors live in the `cursors` system table). On From proto definition (Postgres or ClickHouse), use `from-proto` — cursor handling is built into that command path.

## Advanced Patterns

### Multi-Database Sinks

To stream to multiple databases, use separate packages or sink runs. Modes must match engine rules:

```yaml
# operational: PostgreSQL + Database Changes
sink:
  module: db_out_operational
  type: sf.substreams.sink.sql.v1.Service
  config:
    schema: ./operational-schema.sql
    engine: postgres
```

```yaml
# analytics: ClickHouse + From proto definition (custom proto output, not DatabaseChanges)
sink:
  module: map_analytics
  type: sf.substreams.sink.sql.v1.Service
  config: {}
```

```bash
# Postgres Database Changes
substreams-sink-sql setup "psql://user:pass@postgres:5432/operational" operational.spkg
substreams-sink-sql run "psql://user:pass@postgres:5432/operational" operational.spkg &

# ClickHouse from-proto only
substreams-sink-sql from-proto "clickhouse://default:@clickhouse:9000/analytics" ./analytics-substreams.yaml &
```

### Data Transformation Pipelines

```rust
// Multi-stage data processing
#[substreams::handlers::map]
pub fn extract_events(block: Block) -> Result<RawEvents, Error> {
    // Stage 1: Extract raw events
    extract_raw_blockchain_events(block)
}

#[substreams::handlers::map]
pub fn enrich_events(raw_events: RawEvents) -> Result<EnrichedEvents, Error> {
    // Stage 2: Enrich with metadata, decode parameters
    enrich_with_token_metadata(raw_events)
}

// PostgreSQL Database Changes path only
#[substreams::handlers::map]
pub fn db_out_operational(enriched: EnrichedEvents) -> Result<DatabaseChanges, Error> {
    create_operational_tables(enriched)
}

// ClickHouse / from-proto path: emit your annotated domain proto, not DatabaseChanges
#[substreams::handlers::map]
pub fn map_analytics(enriched: EnrichedEvents) -> Result<AnalyticsOutput, Error> {
    create_analytics_output(enriched)
}
```

## Schema evolution (from-proto)

From-proto DDL is **create-if-not-exists** and **insert-only**. Changing the protobuf (rename column, add required field, change types, fix reserved names) does **not** alter existing tables.

| Change | Local self-managed | Hosted (`substreams-hosted-sink`) |
|---|---|---|
| Safe (no column shape change) | Redeploy spkg / restart sink | `UpdateDeploymentConfig` with new `spkg.url` |
| Column rename / add / reserved-name fix | Drop affected tables (or database) and re-run `from-proto` | Confirm, then **`ResetDeployment` with `drop_schema: true`**, or user drops tables manually |
| Partial failed CREATE left inconsistent tables | Drop and recreate | Same reset / drop |

**Error after a “fixed” spkg:** `NO_SUCH_COLUMN_IN_TABLE` (e.g. code expects `account_keys` but table still has `keys`) → schema drift; reset/drop, not another blind restart.

## Troubleshooting

### Common Issues

**ClickHouse: reserved identifier / `SYNTAX_ERROR` near column name**

```text
SYNTAX_ERROR (62): DB::Exception: Syntax error: failed at position … (',')
… index UInt32, filter_period …
```

**Cause:** proto field named `index` (or another reserved word).  
**Fix:** rename in proto + Rust (`config_index`), rebuild, then drop/recreate tables or hosted reset with `drop_schema: true`.

**ClickHouse: `Primary key must be a prefix of the sorting key`**

```text
BAD_ARGUMENTS: Primary key must be a prefix of the sorting key, but the column
in the position 0 is slot, not id
CREATE TABLE ... PRIMARY KEY (id) ... ORDER BY (slot, id)
```

**Cause:** `primary_key: true` fields do not match the leading `order_by_fields` (e.g. only `id` is PK but ORDER BY starts with `slot`). Common on hosted SQL → ClickHouse when agents optimize for slot range scans without updating PK.

**Fix:** Align proto annotations (rebuild spkg, redeploy). Either:

- `order_by_fields: [id, …]` with only `id` as PK, or
- `order_by_fields: [slot, id, …]` with **both** `slot` and `id` marked `primary_key: true` in that order.

Do not “fix” only the host/DB — the DDL is generated from the spkg proto. Also avoid `partition_fields` that include raw high-cardinality `slot`; use `toYYYYMM(_block_timestamp_)`.

**Connection Problems**:
```bash
# Test database connectivity
psql "postgresql://user:pass@localhost:5432/db" -c "SELECT version();"
clickhouse-client --host localhost --port 9000 --query "SELECT version()"

# Check sink logs
substreams run -s 1000000 -t +1000 db_out --debug
```

**Schema Mismatches**:
```bash
# Compare expected vs actual schema
pg_dump --schema-only dbname > current_schema.sql
diff schema.sql current_schema.sql

# Re-run setup to apply schema changes (drops and recreates in dev mode)
substreams-sink-sql setup "psql://..." my-substreams.spkg
```

**Performance Issues**:
```sql
-- Monitor query performance
EXPLAIN ANALYZE SELECT * FROM erc20_transfers WHERE contract_address = '0x...';

-- Check index usage
SELECT schemaname, tablename, indexname, idx_scan, idx_tup_read, idx_tup_fetch
FROM pg_stat_user_indexes
ORDER BY idx_scan DESC;

-- ClickHouse query profiling
SELECT * FROM system.query_log
WHERE type = 'QueryFinish'
ORDER BY event_time DESC
LIMIT 10;
```

### Data Consistency

**Reorg Handling**:
```rust
// Proper reorg handling in stores
#[substreams::handlers::store]
pub fn store_balances(events: Events, store: StoreSetBigInt) {
    for transfer in events.transfers {
        // Use ordinal for correct ordering within a block
        let key = format!("{}:{}", transfer.contract, transfer.from);
        store.set(transfer.ordinal, &key, &transfer.amount);
    }
}
```

**Cursor Management**:

Cursor state is automatically persisted in the `cursors` table created by `substreams-sink-sql setup`. On restart, the sink resumes from the last committed cursor. In development mode (`--development-mode`), undos are handled automatically for reorg safety.

### Monitoring and Alerting

**Key Metrics**:
```sql
-- PostgreSQL monitoring
SELECT
    schemaname,
    tablename,
    n_tup_ins as inserts,
    n_tup_upd as updates,
    n_tup_del as deletes
FROM pg_stat_user_tables
ORDER BY n_tup_ins DESC;

-- ClickHouse monitoring
SELECT
    table,
    sum(rows) as total_rows,
    sum(bytes_on_disk) as size_bytes
FROM system.parts
WHERE active = 1
GROUP BY table
ORDER BY size_bytes DESC;
```

**Health Checks**:
```bash
#!/bin/bash
# Database health monitoring script

# Check latest block processed
LATEST_BLOCK=$(psql $DSN -t -c "SELECT MAX(block_number) FROM transactions;")
CHAIN_HEAD=$(curl -s https://api.etherscan.io/api?module=proxy&action=eth_blockNumber | jq -r .result)

LAG=$((CHAIN_HEAD - LATEST_BLOCK))
if [ $LAG -gt 100 ]; then
    echo "WARNING: Database is $LAG blocks behind chain head"
    exit 1
fi

echo "Database is healthy, lag: $LAG blocks"
```

## Resources

* [Database Changes Documentation](./references/database-changes.md) — **PostgreSQL only**
* [PostgreSQL Best Practices](./references/postgresql-patterns.md)
* [ClickHouse Optimization Guide](./references/clickhouse-patterns.md) — **From proto definition only**
* [Schema Design Patterns](./references/schema-patterns.md)
* [FROM_PROTO guide (upstream)](https://github.com/streamingfast/substreams-sink-sql/blob/develop/FROM_PROTO.md)
* [ClickHouse from-proto showcase](https://github.com/streamingfast/substreams-sink-clickhouse-showcase)

## Getting Help

* [Substreams Discord](https://discord.gg/streamingfast)
* [SQL Sink Documentation](https://docs.substreams.dev/how-to-guides/sinks/sql)
* [GitHub Issues](https://github.com/streamingfast/substreams-sink-sql/issues)