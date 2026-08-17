---
name: substreams-sql
description: Expert knowledge for building SQL database sinks from Substreams. Covers sink-type selection, PostgreSQL vs ClickHouse, Database Changes vs From-proto modes, primary keys, delta updates, and schema evolution.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.6.0
  author: StreamingFast
  documentation: https://substreams.streamingfast.io
---

# Substreams SQL Expert

Build the Substreams **module** that feeds a SQL sink (`substreams sink postgres` / `substreams sink clickhouse`) — PostgreSQL or ClickHouse.

**Scope:** this skill covers module design, proto/schema shape, and mapping mode. For *running* a sink yourself use `substreams-sink-deploy-local`; for StreamingFast-hosted sinks use `substreams-hosted-sink`.

Facts below are verified against the built-in SQL sink in `substreams` **v1.20.2** and `substreams-database-change` **4.0.0**. The standalone `substreams-sink-sql` binary (last v4.13.1) is deprecated — same engine, different CLI surface; see the [migration guide](https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/migration.md).

## Pre-flight: ask before building

**Do not write manifests, schemas, or Rust until each applicable step is answered.** One question per turn, explicit choices, always include **"Other / enter a custom answer"**. Skip steps the user already answered.

### Step 1 — Sink type

| Choice | Product | Destination |
|---|---|---|
| **SQL** | built into the [`substreams` CLI](https://github.com/streamingfast/substreams) (`sink postgres` / `sink clickhouse`) | PostgreSQL or ClickHouse |
| **KV** | [`substreams-sink-kv`](https://github.com/streamingfast/substreams-sink-kv) | Key-value stores |
| **Files** | [`substreams-sink-files`](https://github.com/streamingfast/substreams-sink-files) | Local FS / S3 / GCS |
| **Other / custom** | — | Re-route or clarify |

KV or Files → hand off to `substreams-sink` / `substreams-sink-deploy-local`. Only continue on **SQL**.

**Who runs it?** (separate turn) Self-managed → `substreams-sink-deploy-local` after the module is built. StreamingFast-hosted → `substreams-hosted-sink`. Module design (this skill) is shared; ops path is not.

### Step 2 — Mutable state? (this drives everything)

Ask what the tables must do, *then* derive engine and mode. This is the real constraint:

> **UPDATE / DELETE / upsert semantics and delta aggregations require PostgreSQL + Database Changes.**
> No other combination gives you mutable rows.

| Need | Engine + mode |
|---|---|
| Mutable rows (balances, positions, current state) | **PostgreSQL + Database Changes** |
| Aggregations in-DB (counters, candles, volumes) | **PostgreSQL + Database Changes** (delta ops) |
| Insert-only mirror of events, high volume, OLAP | **ClickHouse + from-proto** (recommended) |
| Insert-only mirror, modest volume, already on PG | **PostgreSQL + from-proto** |

### Step 3 — Remaining inputs (one per turn)

Chain + contract/protocol · data shape & primary keys · block range (`initialBlock` + test window) · aggregations needed?

## Capability matrix (correct as of substreams v1.20.2)

**Both modes work on both engines** — `ClickhouseDialect` implements the full Database Changes dialect, and `setup` / the engine command accept a ClickHouse DSN. The limits are about *capability*, not support:

| | PostgreSQL | ClickHouse |
|---|---|---|
| **Database Changes** | Full: INSERT / UPDATE / DELETE / upsert, delta ops, DB-side reorg handling | Runs, but **insert-only** (`OnlyInserts()=true`), **no reorg management**, **no delta ops**, duplicate PKs allowed |
| **From proto** | Insert-only. Identifiers quoted. FK + `child_of` honored | Insert-only. `ReplacingMergeTree(_version_, _deleted_)`. `clickhouse_table_options` **required** |

The mode is **auto-detected from the output module's proto type** — `sf.substreams.sink.database.v1.DatabaseChanges` selects Database Changes, anything else selects from-proto. There is no `from-proto` subcommand and no mode flag.

**ClickHouse + Database Changes is rarely the right choice** — it is insert-only anyway, so it buys nothing over from-proto while losing schema generation. If you must: `Revert()` returns `"clickhouse driver does not support reorg management"` and the history-table path panics, so you **must** pass `--undo-buffer-size > 0` (the default `0` enables DB-side reorg handling and will fail).

## Prerequisites

```bash
brew install streamingfast/tap/substreams                   # SQL sink included, v1.20.2+
docker pull ghcr.io/streamingfast/substreams
```
Binaries: [GitHub Releases](https://github.com/streamingfast/substreams/releases).

> The SQL sink ships **inside the `substreams` CLI** — `substreams sink postgres` and `substreams sink clickhouse`. The standalone `substreams-sink-sql` binary (and the older `substreams-sink-postgres`) are deprecated.

### DSN formats

Accepted schemes: **`psql`, `postgres`, `clickhouse`, `parquet`**. **`postgresql://` is rejected** (`invalid scheme postgresql`) — note this is the opposite of what the `psql` CLI accepts, so a DSN that works with `psql` may not work here and vice-versa.

```bash
psql://user:pass@localhost:5432/db?sslmode=disable          # sslmode=disable: localhost only
clickhouse://default:@localhost:9000/default                # native TCP
clickhouse://default:pw@xxx.clickhouse.cloud:9440/default?secure=true   # ClickHouse Cloud
```

- **ClickHouse HTTP ports 8123 / 8443 are hard-rejected.** Use native TCP **9000** (or **9440** secure).
- The DSN port defaults to **5432 when omitted — even for ClickHouse**. Always set it explicitly.

## Database Changes (CDC)

Module emits `DatabaseChanges`; the sink applies each change. **Use PostgreSQL** (see matrix above).

The proto comes from the official spkg — do not define your own.

```rust
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_database_change::tables::Tables;

#[substreams::handlers::map]
pub fn db_out(events: Events) -> Result<DatabaseChanges, substreams::errors::Error> {
    let mut tables = Tables::new();

    for transfer in events.transfers {
        // Composite PK: array of (column, value) tuples matching schema.sql
        // names AND order. Never string-concat a multi-column PK.
        let log_index = transfer.log_index.to_string();
        tables
            .create_row("transfers", [
                ("tx_hash", transfer.tx_hash.as_str()),
                ("log_index", log_index.as_str()),
            ])
            .set("from_addr", transfer.from)
            .set("amount", transfer.amount);
    }

    for balance in events.balance_changes {
        // Single-column PK: a plain string key
        tables
            .update_row("balances", balance.address)
            .set("balance", balance.new_balance);
    }

    Ok(tables.to_database_changes())
}
```

> The Rust path `pb::database::DatabaseChanges` is **deprecated but still present** in v4. The **proto FQN `sf.substreams.sink.database.v1.DatabaseChanges` has been stable since v1** — only the Rust module path changed in v4. A v3 spkg emits the same FQN.

### Primary keys

The key argument is `K: Into<PrimaryKey>` — either a single string (`Single`) or a **fixed-size array** of `(column, value)` tuples (`Composite`). Names and order must match `schema.sql`'s `PRIMARY KEY (...)`. There is no `Vec` impl, and arrays must be homogeneous (all `&str` or all `String`). Full impl list in [database-changes.md](./references/database-changes.md).

### Delta updates (aggregations)

Atomic in-DB modification, no read-modify-write. **PostgreSQL only**, requires crate **>= 4.0.0** (these ops did not exist in 3.x). Present in every built-in sink release; on the deprecated standalone binary it needed **>= v4.12.0**.

```rust
tables.upsert_row("aggregates", [("day", day.as_str()), ("token", token.as_str())])
    .set_if_null("first_seen", &timestamp)  // COALESCE(col, value)
    .set("last_seen", &timestamp)           // col = value
    .max("highest", price)                  // GREATEST(col, value)
    .min("lowest", price)                   // LEAST(col, value)
    .add("total", volume.as_str())          // COALESCE(col,0) + value
    .sub("outflow", amount.as_str());       // COALESCE(col,0) - value
```

**Trait bounds differ — the main footgun.** `add`/`sub` need `NumericAddable` (`String`, `&str`, ints, `BigDecimal`/`BigInt` and refs — but **not `&String`**, so pass `value.as_str()`). `max`/`min` need `NumericComparable`, which is **not** implemented for `String`/`&str`. `add`/`sub` also panic at runtime on non-numeric strings, and mixing incompatible ops on one column (e.g. `max` then `add`) panics. Ordinals are auto-managed.

### Manifest

```yaml
specVersion: v0.1.0
package:
  name: my_substreams_sql
  version: 1.6.0
  url: https://github.com/myorg/my-substreams-sql   # set both to avoid build warnings
  description: SQL sink substreams for <protocol>

imports:
  database: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v4.0.0/substreams-sink-database-changes-v4.0.0.spkg
  sql: https://github.com/streamingfast/substreams-sink-sql/releases/download/protodefs-v1.0.7/substreams-sink-sql-protodefs-v1.0.7.spkg

protobuf:
  excludePaths: [sf/substreams, google]

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
  type: sf.substreams.sink.sql.service.v1.Service   # sf.substreams.sink.sql.v1.Service is DEPRECATED
  config:
    schema: ./schema.sql
    engine: postgres        # inert for the CLI (dialect comes from the DSN); matters for hosted deploys
```

```toml
[dependencies]
substreams-database-change = "4"   # 4.0.0; prost 0.13
```

> **Graph output is a different sink.** For The Graph use `sf.substreams.sink.entity.v1.EntityChanges`, not `DatabaseChanges` — see `substreams-sink`. Don't add `substreams-entity-change = "1"`: it pins `prost 0.11` / `substreams 0.5`, so its `BigDecimal`/`Hex` types are *distinct types* from the ones in `substreams 0.7` and won't interoperate.

### Run

```bash
substreams build
export SUBSTREAMS_SINK_DSN="$DSN"                                 # or pass --dsn on each command
substreams sink postgres setup my-substreams-sql-v0.1.0.spkg      # system tables + schema.sql
substreams sink postgres       my-substreams-sql-v0.1.0.spkg      # runs the sink — no `run` subcommand

# Short smoke ranges won't flush at the default batch size (1000 blocks):
substreams sink postgres ./pkg.spkg -s 18000000 -t +100 --batch-block-flush-interval=1
```

Cursors live in the **`cursors`** table created by `setup` (`--cursors-table`). Resume is automatic.

## From proto definition

Your **own** annotated proto; the sink generates DDL and inserts rows. **Insert-only** from the module's perspective.

```proto
syntax = "proto3";
package transfers;

import "sf/substreams/sink/sql/schema/v1/schema.proto";   // proto package is bare `schema`

message Output { repeated Transfer transfers = 1; }

message Transfer {
  option (schema.table) = {
    name: "transfers"
    clickhouse_table_options: {                  // REQUIRED for ClickHouse, ignored by Postgres
      order_by_fields: [{ name: "tx_hash" }, { name: "log_index" }]
      partition_fields: [{ name: "_block_timestamp_", function: toYYYYMM }]
    }
  };

  string tx_hash   = 1 [(schema.field) = { primary_key: true }];   // AT MOST ONE per message
  uint32 log_index = 2;
  string amount    = 3 [(schema.field) = { convertTo: { uint256{} } }];
}
```

### Hard rule: exactly one primary key

**from-proto supports at most ONE `primary_key: true` field per table message.** `Table.PrimaryKey` is a scalar; a second one fails the build with:

```text
multiple field mark has primary keys are not supported
```

There are **no composite primary keys in from-proto** — if you need one, use Database Changes on PostgreSQL, or synthesize a single unique key column (e.g. `id = "{tx_hash}-{log_index}"`). The PK is also **optional**; many tables have none.

### ClickHouse: PK must prefix ORDER BY

ClickHouse (not the sink) enforces that `PRIMARY KEY` is a prefix of `ORDER BY`. Since the PK is a single column, the rule reduces to:

> **`order_by_fields[0]` must be the `primary_key: true` field.** Extra sort columns follow it.

```text
DB::Exception: Primary key must be a prefix of the sorting key, but the column
in the position 0 is slot, not id
```

Fix: either make `id` the first `order_by_field`, or mark `slot` (the leading sort column) as the PK instead — not both.

### Annotation reference

`clickhouse_table_options` has exactly three fields: **`order_by_fields`** (`{name, descending, function}`, at least one **required**), `partition_fields` (`{name, function}`, defaults to `toYYYYMM(_block_timestamp_)`), and `index_fields`. The `function` enum is `toYYYYMM`, `toYYYYDD`, `toYear`, `toMonth`, `toDate`, `toStartOfMonth` — but ⚠️ as of substreams v1.20.2 `toStartOfMonth` is **silently ignored** and `toYYYYDD` emits `toYYYYMMDD`, so stick to `toYYYYMM`. Details in [clickhouse-patterns.md](./references/clickhouse-patterns.md).

`(schema.field)` options: `primary_key`, `foreign_key`, `convertTo`.
- `convertTo`: `int128{}`, `uint128{}`, `int256{}`, `uint256{}`, `decimal128{scale: N}`, `decimal256{scale: N}` — maps a proto `string` to a wide numeric column.
- `foreign_key` / `child_of` format is **`"table_name on field_name"`** (not `table.column` — the upstream proto comment is wrong). `foreign_key` is **Postgres-only**; ClickHouse ignores it.

### Injected columns

| Column | Postgres from-proto | ClickHouse from-proto |
|---|---|---|
| `_block_number_`, `_block_timestamp_` | ✅ | ✅ |
| `_version_`, `_deleted_` | ❌ **not present** | ✅ |

So `WHERE _deleted_ = 0` is a **ClickHouse-only** filter — it errors on Postgres. ClickHouse tables are `ReplacingMergeTree(_version_, _deleted_)`; filter `_deleted_ = 0` in queries and MVs.

### Manifest (from-proto)

```yaml
network: solana   # or mainnet, etc.

protobuf:
  files: [events.proto]
  descriptorSets:
    - module: buf.build/streamingfast/substreams-sink-sql   # resolves schema.proto + deps
  importPaths: [./proto]
  excludePaths: [sf/substreams, google]

modules:
  - name: map_transfer
    kind: map
    inputs:
      - source: sf.ethereum.type.v2.Block
    output:
      type: proto:transfers.Output   # your proto — NOT DatabaseChanges
```

Without `descriptorSets`, the build cannot resolve `sf/substreams/sink/sql/schema/v1/schema.proto`. The `sink:` **service config** is optional for from-proto (the CLI falls back to an empty service) — but the sink still has to resolve a module, so either declare `sink: module:` or pass the module as a trailing positional argument.

### Run

```bash
# Same command as Database Changes — the mode follows the output module's proto type.
substreams sink postgres ./substreams.yaml --dsn "$DSN"              # module from sink: module:
substreams sink postgres ./substreams.yaml map_transfer --dsn "$DSN" # or name it explicitly

# Optional: create the tables ahead of time (idempotent; the sink also does this on startup).
# `setup` takes a manifest only — it cannot be given a module, so it REQUIRES sink: module:
substreams sink postgres setup ./substreams.yaml --dsn "$DSN"

# Smoke tests: from-proto batches 25 blocks by default.
# NOTE: --batch-block-flush-interval has no effect here — it is a Database Changes flag.
substreams sink postgres ./substreams.yaml --dsn "$DSN" --block-batch-size=1
```

**Cursor storage differs by engine** — the `cursors` table belongs to Database Changes only:

| Engine | from-proto cursor |
|---|---|
| PostgreSQL | auto-created **`_cursor_` table** (no `setup` needed) |
| ClickHouse | **local file**, default `cursor.txt` (`--cursor-file-path`) |

⚠️ The ClickHouse cursor is *on disk, not in the database*. Losing the file loses your position. The schema hash is likewise a local file (`--sink-info-folder`). Both flags lost their `--clickhouse-` prefix and exist only on `substreams sink clickhouse`.

### Reorg handling in from-proto

- **ClickHouse:** inserts tombstone rows with `_deleted_ = true`.
- **PostgreSQL:** `DELETE FROM _blocks_ WHERE number > N`, cascading via an `ON DELETE CASCADE` FK. **`--no-constraints`, or not importing `schema.proto`, drops that FK and silently disables reorg cleanup.**

## Schema evolution (from-proto)

DDL is `CREATE TABLE IF NOT EXISTS` and **the migration path is an unimplemented stub**. On a proto change the sink detects drift, does nothing, and streams against the **old** table — no DDL, no error.

| Change | Self-managed | Hosted |
|---|---|---|
| No column shape change | Redeploy / restart | `UpdateDeploymentConfig` with new `spkg.url` |
| Rename / add / retype a column | **Drop the tables** and re-run the sink (or `setup`) | `ResetDeployment` with `drop_schema: true` |

Symptom of drift: `NO_SUCH_COLUMN_IN_TABLE` after deploying a "fixed" spkg. Drop and recreate — another restart won't help. On ClickHouse, deleting the sink-info folder also silently re-triggers this (`IF NOT EXISTS` no-ops on the existing table).

## Troubleshooting

**`multiple field mark has primary keys are not supported`** → two `primary_key: true` fields in one message. from-proto allows one; synthesize a composite string key or move to Database Changes.

**`Primary key must be a prefix of the sorting key`** → the PK field isn't `order_by_fields[0]`. Fix the proto and rebuild — the DDL comes from the spkg, not the database.

**ClickHouse `SYNTAX_ERROR (62)` near a column name** → the column is named **`index`**, which ClickHouse's `CREATE TABLE` parser reads as an index declaration. Rename it (`config_index`) in proto and Rust. This is *the* reserved word that bites — `keys`, `value`, `status`, `order`, `group`, `table` etc. all work unquoted (verified on CH 26.6.1), and Postgres quotes identifiers so it's a non-issue there. After a rename you must drop/recreate — `IF NOT EXISTS` won't add the column.

**`clickhouse table options not set for table ...`** → stale error text from old docs; the real message names `clickhouse_table_options` as required. Add `order_by_fields`.

**Rows never appear on a short test range** → batching. Database Changes: `--batch-block-flush-interval=1`. From-proto: `--block-batch-size=1`.

**`invalid scheme postgresql`** → use `psql://` or `postgres://`.

**ClickHouse connection hangs/EOF** → HTTP port. Use 9000/9440; add `?secure=true` on Cloud.

**Delta ops silently not applying** → crate < 4.0.0, a standalone binary older than v4.12.0, or you're on ClickHouse (Postgres-only).

## Anti-patterns

- **Don't hand-write DDL for from-proto tables.** The sink generates them; a pre-created table is silently accepted by `IF NOT EXISTS` and then every insert fails on the missing injected columns.
- **Don't add `CHECK (amount > 0)` or `CHECK (from != to)`** to sink schemas — zero-value and self-transfers are legal on-chain and will abort the batch.
- **Don't use `SERIAL` / `gen_random_uuid()` PKs.** Substreams replays must be deterministic; the sink supplies the key.
- **Don't skip already-processed blocks** with a store of the last block number. The sink owns the cursor and may undo/replay; a high-water mark breaks reorgs and backfills.
- **Don't aggregate with row triggers.** They only increment, so reorg DELETEs corrupt them. Use delta updates.
- **Postgres materialized views do not auto-refresh** — Substreams won't do it, and `REFRESH ... CONCURRENTLY` cannot run inside a trigger. Schedule it (cron/pg_cron) or use delta-update tables.

## Resources

* [Database Changes reference](./references/database-changes.md) — Rust `Tables` API, delta ops, schema.sql
* [ClickHouse reference](./references/clickhouse-patterns.md) — from-proto DDL, ORDER BY, MVs
* [Relational mappings / from-proto guide (upstream)](https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/relational-mappings.md)
* [ClickHouse from-proto showcase](https://github.com/streamingfast/substreams-sink-clickhouse-showcase)
* [SQL Sink docs](https://docs.substreams.dev/how-to-guides/sinks/sql) · [Migration guide](https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/migration.md) · [Discord](https://discord.gg/streamingfast) · [Issues](https://github.com/streamingfast/substreams/issues)
