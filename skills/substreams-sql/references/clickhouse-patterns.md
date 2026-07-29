# ClickHouse Reference (from-proto)

Deep reference for ClickHouse sinks. See `SKILL.md` for mode selection.

Verified against the built-in SQL sink in `substreams` **v1.20.2** and ClickHouse **26.6.1**.

## Mode

ClickHouse supports **both** mapping modes, but **from-proto is the right one**. Database Changes on ClickHouse is insert-only (`OnlyInserts()=true`), has no delta ops, and cannot do DB-side reorg management (`Revert()` errors, the history path panics) — so it gives up schema generation for nothing. See `database-changes.md` if you must.

## Generated DDL

The sink builds each table from your annotated proto — **you never hand-write it**:

```sql
CREATE TABLE IF NOT EXISTS <db>.<table>
(
    <your proto fields>,
    _block_number_    UInt64,
    _block_timestamp_ timestamp,
    _version_         Int64,
    _deleted_         bool
)
ENGINE = ReplacingMergeTree(_version_, _deleted_)
PRIMARY KEY (<the single primary_key field>)
ORDER BY (<order_by_fields>)
PARTITION BY (<partition_fields>)
SETTINGS allow_experimental_replacing_merge_with_cleanup = 1;
```

`PARTITION BY toYYYYMM(_block_timestamp_)` is auto-prepended unless you declare a `_block_timestamp_` partition field yourself.

> **Never pre-create these tables.** DDL is `CREATE TABLE IF NOT EXISTS`, so a hand-written table is silently accepted and then every insert fails on the missing `_version_` / `_deleted_` / `_block_number_` columns.

All four injected columns are **ClickHouse-only**. Postgres from-proto gets `_block_number_` and `_block_timestamp_` *only* — `_version_`/`_deleted_` do not exist there, so `WHERE _deleted_ = 0` is not portable.

## Exactly one primary key

**from-proto accepts at most ONE `primary_key: true` field per table message.** `Table.PrimaryKey` is a scalar; a second one fails the build:

```text
multiple field mark has primary keys are not supported
```

Composite primary keys are impossible in from-proto. If you need one, synthesize a single unique column (`id = "{signature}-{ordinal}"`) or use Database Changes on PostgreSQL. The PK is also optional — tables without one are fine.

## PK must prefix ORDER BY

| DDL clause | Source |
|---|---|
| `PRIMARY KEY` | the single `[(schema.field) = { primary_key: true }]` field |
| `ORDER BY` | `clickhouse_table_options.order_by_fields`, in order |
| `PARTITION BY` | `partition_fields` |

ClickHouse — not the sink — requires the primary key to be a prefix of the sorting key. With a scalar PK that reduces to one rule:

> **`order_by_fields[0]` must be the `primary_key: true` field.** Extra sort columns follow it.

```text
DB::Exception: Primary key must be a prefix of the sorting key, but the column
in the position 0 is slot, not id
```

Pick one, not both:

```proto
// Identity-first (safe default): PK is the unique row key, listed first.
message Swap {
  option (schema.table) = {
    name: "raydium_clmm_swaps"
    clickhouse_table_options: {
      order_by_fields: [{ name: "id" }, { name: "slot" }]   // secondary sort AFTER the PK
      partition_fields: [{ name: "_block_timestamp_", function: toYYYYMM }]
    }
  };
  string id   = 1 [(schema.field) = { primary_key: true }];  // e.g. signature-ordinal
  uint64 slot = 2;
}

// Slot-first (only if you need range scans by slot): PK moves to slot.
message SwapBySlot {
  option (schema.table) = {
    name: "raydium_clmm_swaps"
    clickhouse_table_options: {
      order_by_fields: [{ name: "slot" }, { name: "id" }]
      partition_fields: [{ name: "_block_timestamp_", function: toYYYYMM }]
    }
  };
  uint64 slot = 1 [(schema.field) = { primary_key: true }];  // NOT id — only one PK allowed
  string id   = 2;
}
```

Pre-deploy checklist:

1. Exactly one `primary_key: true` in each table message.
2. That field is `order_by_fields[0]`.
3. `partition_fields` are coarse (`toYYYYMM(_block_timestamp_)`) — never raw `slot`/`block_number`, which explodes the partition count.
4. No column named `index` (see below).

## Table options

`clickhouse_table_options` is **required** on ClickHouse. Omitting it fails with:

```text
schema annotation 'clickhouse_table_options' is required in table annotation
'option (schema.table) = { name: "...", ... }'
```

(An older error string, `clickhouse table options not set for table "..."`, still appears in upstream docs but no longer exists in the code.) Empty options fail separately: `... don't have any 'order_by_fields'. Require at least 1`.

Exactly three fields:

| Field | Shape |
|---|---|
| `order_by_fields` | `{name, descending, function}` — **>= 1 required** |
| `partition_fields` | `{name, function}` |
| `index_fields` | `{name, field_name, type, granularity, function}` |

`function` enum: `toYYYYMM`, `toYYYYDD`, `toYear`, `toMonth`, `toDate`, `toStartOfMonth`.
`index_fields.type`: `minmax`, `set`, `ngrambf_v1`, `tokenbf_v1`, `bloom_filter`.

⚠️ **Upstream bugs, still present in substreams v1.20.2:** `toStartOfMonth` hits an empty switch case and is **silently ignored** (emits the raw field); `toYYYYDD` emits `toYYYYMMDD`. Stick to `toYYYYMM`.

## Column naming: only `index` is a problem

The ClickHouse from-proto dialect emits **bare, unquoted identifiers**. In practice exactly one word breaks (verified live on CH 26.6.1):

| Name | Result |
|---|---|
| `index` | ❌ `SYNTAX_ERROR (62)` — the `CREATE TABLE` parser reads a leading `INDEX` token as an index declaration, in *any* position |
| `keys`, `value`, `status`, `order`, `group`, `table`, `key`, `data`, `type`, `name`, `check`, `all`, `from`, `select`, `where` | ✅ all fine unquoted |

Rename `index` → `config_index` in **proto and Rust**. Postgres from-proto quotes identifiers, so none of this applies there.

After any rename, `CREATE TABLE IF NOT EXISTS` will **not** add the column to an existing table — drop and recreate (see below).

## Cursors live on disk

**The ClickHouse from-proto cursor is a local file, not a database table** — default `cursor.txt`, set via `--cursor-file-path`. The schema hash is likewise a local file under `--sink-info-folder`. Both flags dropped their old `--clickhouse-` prefix and exist only on `substreams sink clickhouse`.

| | Cursor location |
|---|---|
| ClickHouse + from-proto | **local file** (`cursor.txt`) |
| Postgres + from-proto | auto-created `_cursor_` table |
| Either + Database Changes | `cursors` table (created by `setup`) |

Consequences: losing the file loses your stream position, and losing the sink-info folder makes the sink think the schema is new — `IF NOT EXISTS` then no-ops against the existing table and inserts fail on drift. Mount both on durable storage.

## Reorgs

from-proto on ClickHouse handles undos by **inserting tombstones** rather than deleting:

```sql
INSERT INTO <table> SELECT ..., true AS _deleted_ FROM <table>
WHERE _block_number_ > <last_valid> AND _deleted_ != 1
```

`ReplacingMergeTree` collapses them on merge, which is asynchronous — so **always filter `_deleted_ = 0`** in queries and materialized views. Use `FINAL` only when you need immediate correctness; it is expensive.

## Schema evolution

DDL is create-if-not-exists and **the migration path is an unimplemented stub**: on drift the sink detects the schema-hash change, does nothing, and streams against the old table. No DDL, no error, no warning — then inserts fail with `NO_SUCH_COLUMN_IN_TABLE`.

To change a column: drop the affected tables and re-run the sink (or `substreams sink clickhouse setup`), or hosted `ResetDeployment` with `drop_schema: true`. Restarting a "fixed" spkg on its own does nothing.

## Querying

```sql
-- Partition pruning + tombstone filter
SELECT contract_address, count() AS transfers, sum(amount) AS volume
FROM erc20_transfers
WHERE toYYYYMM(_block_timestamp_) = 202401
  AND _deleted_ = 0
GROUP BY contract_address;
```

Materialized views read from the sink's base tables — create them after the sink has created those tables:

```sql
CREATE MATERIALIZED VIEW hourly_transfer_stats
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(hour)
ORDER BY (contract_address, hour)
AS SELECT
    contract_address,
    toStartOfHour(_block_timestamp_) AS hour,
    count()      AS transfer_count,
    sum(amount)  AS total_volume
FROM erc20_transfers
WHERE _deleted_ = 0
GROUP BY contract_address, hour;
```

A ClickHouse MV is an insert trigger: it only sees **newly inserted** rows, so it will not backfill history, and it sees tombstone inserts as ordinary rows — the `_deleted_ = 0` filter is what keeps it correct.

Two ClickHouse-isms worth knowing: window functions are `lagInFrame`/`leadInFrame` (there is no `lag`/`lead`), and `uniq(a, b)` counts distinct *tuples* — for distinct values across two columns use `uniq(arrayJoin([a, b]))`.

## Connection

```bash
substreams sink clickhouse ./substreams.yaml --dsn "clickhouse://default:@localhost:9000/default"
```

The from-proto mode is auto-detected from the output module's proto type — there is no separate `from-proto` command to invoke.

- Native TCP **9000** / **9440** only. **HTTP 8123 / 8443 are hard-rejected.**
- ClickHouse Cloud: port 9440 + `?secure=true` (without it you get opaque `read: EOF` errors).
- The DSN port **defaults to 5432 when omitted, even for ClickHouse** — always set it.
- Smoke tests: `--block-batch-size=1` (default 25). `--batch-block-flush-interval` is a Database Changes flag and has no effect in from-proto mode.

```yaml
# docker-compose.yml
services:
  clickhouse:
    image: clickhouse/clickhouse-server:latest
    ports:
      - "9000:9000"   # native TCP — what the sink uses
      - "8123:8123"   # HTTP — clients only, rejected by the sink
    volumes:
      - clickhouse_data:/var/lib/clickhouse

volumes:
  clickhouse_data:
```
