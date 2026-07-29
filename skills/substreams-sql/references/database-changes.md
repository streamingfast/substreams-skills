# Database Changes (CDC) Reference

Deep reference for the `DatabaseChanges` mapping mode. See `SKILL.md` for mode selection.

Verified against `substreams-database-change` **4.0.0** and the built-in SQL sink in `substreams` **v1.20.2**.

## Engine support

Works on **both** PostgreSQL and ClickHouse, but only PostgreSQL is generally useful:

| | PostgreSQL | ClickHouse |
|---|---|---|
| INSERT | ✅ | ✅ |
| UPDATE / DELETE / upsert | ✅ | ❌ — `OnlyInserts()=true`, every op becomes an insert |
| Delta ops (`add`/`sub`/`min`/`max`/`set_if_null`) | ✅ | ❌ |
| DB-side reorg handling | ✅ | ❌ — `Revert()` errors; history path panics |
| Duplicate PKs | rejected | allowed (`AllowPkDuplicates()=true`) |

On ClickHouse you must pass `--undo-buffer-size > 0` to `run` (the default `0` turns on DB-side reorg handling, which ClickHouse cannot do). Since the mode is insert-only there anyway, prefer **from-proto** for ClickHouse.

## Setup

```toml
[dependencies]
substreams-database-change = "4"   # 4.0.0 — prost 0.13, substreams ^0.7.3
```

Import the official spkg; do not define the proto yourself:

```yaml
imports:
  database: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v4.0.0/substreams-sink-database-changes-v4.0.0.spkg
```

Module output type:

```yaml
output:
  type: proto:sf.substreams.sink.database.v1.DatabaseChanges
```

> This FQN has been **stable since v1**. v4 renamed only the *Rust module path*: use
> `substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges`.
> The old `pb::database::DatabaseChanges` is a deprecated type alias — it still compiles in v4.

## The `Tables` API

```rust
pub fn new() -> Self
pub fn create_row<K: Into<PrimaryKey>>(&mut self, table: &str, key: K) -> &mut Row
pub fn update_row<K: Into<PrimaryKey>>(&mut self, table: &str, key: K) -> &mut Row
pub fn upsert_row<K: Into<PrimaryKey>>(&mut self, table: &str, key: K) -> &mut Row
pub fn delete_row<K: Into<PrimaryKey>>(&mut self, table: &str, key: K) -> &mut Row
pub fn to_database_changes(self) -> DatabaseChanges   // consumes self
```

Row methods:

```rust
pub fn set<T: ToDatabaseValue>(&mut self, name: &str, value: T) -> &mut Self
pub fn set_if_null<T: ToDatabaseValue>(&mut self, name: &str, value: T) -> &mut Self
pub fn add<T: NumericAddable>(&mut self, name: &str, value: T) -> &mut Self
pub fn sub<T: NumericAddable>(&mut self, name: &str, value: T) -> &mut Self
pub fn max<T: NumericComparable>(&mut self, name: &str, value: T) -> &mut Self
pub fn min<T: NumericComparable>(&mut self, name: &str, value: T) -> &mut Self
```

Ordinals are assigned automatically on first touch and sorted in `to_database_changes()`. The field is private — you cannot and need not manage them.

## Primary keys

`PrimaryKey` is an **enum**, not a trait:

```rust
pub enum PrimaryKey {
    Single(String),
    Composite(BTreeMap<String, String>),
}
```

`Into<PrimaryKey>` impls:

| Input | Result |
|---|---|
| `&str` / `String` / `&String` | `Single` |
| `[(K, &str); N]` where `K: AsRef<str>` | `Composite` |
| `[(K, String); N]` where `K: AsRef<str>` | `Composite` |

```rust
// ✅ single
tables.update_row("balances", address.as_str());

// ✅ composite — fixed-size array, homogeneous value type
let log_index = transfer.log_index.to_string();
tables.create_row("transfers", [
    ("tx_hash", transfer.tx_hash.as_str()),
    ("log_index", log_index.as_str()),
]);

// ❌ no Vec impl
tables.create_row("transfers", vec![("tx_hash", "0x..")]);

// ❌ mixed &str / String in one array
tables.create_row("transfers", [("tx_hash", "0x.."), ("log_index", log_index)]);

// ❌ never concat a multi-column PK into one string
tables.create_row("transfers", format!("{}-{}", tx_hash, log_index));
```

Composite key names and order **must match `schema.sql`**:

| `schema.sql` | Rust key |
|---|---|
| `PRIMARY KEY (tx_hash, log_index)` | `[("tx_hash", ..), ("log_index", ..)]` — same names, same order |
| `PRIMARY KEY (address)` | `address.as_str()` |

Note: `tables::PrimaryKey` is distinct from the generated proto `pb::...::table_change::PrimaryKey`. Both exist; you want the `tables::` one.

## Delta updates

**PostgreSQL only**, crate **>= 4.0.0** (these methods do not exist in 3.x; on the deprecated standalone binary the sink also had to be >= v4.12.0).

Push aggregation into the database instead of maintaining store modules:

```rust
tables.upsert_row("daily_volume", [
        ("day", day.as_str()),
        ("token", token.as_str()),
    ])
    .set_if_null("first_seen", &timestamp)   // COALESCE(col, value) — first write wins
    .set("last_seen", &timestamp)            // col = value
    .max("high", price)                      // GREATEST(col, value)
    .min("low", price)                       // LEAST(col, value)
    .add("volume", amount.as_str())          // COALESCE(col, 0) + value
    .add("trades", 1i64)
    .sub("outflow", withdrawn.as_str());     // COALESCE(col, 0) - value
```

### Trait bounds — the main source of compile errors

| Trait | Implemented for | **Not** implemented for |
|---|---|---|
| `NumericAddable` (`add`, `sub`) | `String`, `&str`, integers, `isize`/`usize`, `BigDecimal`, `&BigDecimal`, `BigInt`, `&BigInt` | **`&String`** |
| `NumericComparable` (`max`, `min`) | integers, `isize`/`usize`, `BigDecimal`, `&BigDecimal`, `BigInt`, `&BigInt` | **`String`, `&str`** |

```rust
.add("volume", &amount)          // ❌ &String: no NumericAddable impl
.add("volume", amount.as_str())  // ✅ preferred — zero-alloc
.add("volume", amount.clone())   // ✅ compiles, needless allocation
.max("high", price_str)          // ❌ String has no NumericComparable impl
.max("high", &price_bigdecimal)  // ✅
```

### Runtime panics

- `add`/`sub` parse string values with `BigDecimal::from_str` → **panic on non-numeric input**
  (`"add/sub() requires a valid numeric value, got: ..."`). Validate upstream.
- Mixing incompatible operations on the same column in one row (e.g. `max` then `add`) **panics**.

## schema.sql

Applied by `substreams sink postgres setup`. Keep it minimal — it must tolerate real chain data and the sink's batched, per-table flush order.

```sql
CREATE TABLE IF NOT EXISTS transfers (
    tx_hash     VARCHAR(66)   NOT NULL,
    log_index   INTEGER       NOT NULL,
    from_addr   VARCHAR(42)   NOT NULL,
    to_addr     VARCHAR(42)   NOT NULL,
    amount      NUMERIC(78,0) NOT NULL,   -- NOT BIGINT: uint256 overflows
    block_num   BIGINT        NOT NULL,
    PRIMARY KEY (tx_hash, log_index)      -- must match create_row's key array
);

CREATE INDEX idx_transfers_from ON transfers(from_addr);
```

**Do not:**

- `CHECK (amount > 0)` / `CHECK (from_addr != to_addr)` — zero-value and self-transfers are legal
  on-chain; the first one aborts the batch.
- `SERIAL` / `gen_random_uuid()` primary keys — replays must be deterministic, and the module
  supplies the key.
- Cross-table `FOREIGN KEY`s — the sink flushes per table and does not guarantee parents land
  before children.
- Row triggers that increment aggregates — reorg DELETEs never decrement them. Use delta updates.

`setup` also creates the **`cursors`** table (`--cursors-table` to rename). Cursor resume is automatic; never hand-roll a last-block-processed store in the module.

## Operations on the wire

The proto `Operation` enum is `UNSET` / `CREATE` / `UPDATE` / `DELETE`. **There is no `Operation::Upsert`** — `upsert_row()` is a `Tables`-level helper, not a wire operation.

## Testing

```rust
use substreams_database_change::pb::sf::substreams::sink::database::v1::Operation;

#[test]
fn emits_transfer_row() {
    let changes = db_out(test_events()).unwrap();
    let change = changes.table_changes.iter().find(|c| c.table == "transfers").unwrap();

    // `operation` is a prost i32 — compare against the enum discriminant
    assert_eq!(change.operation, Operation::Create as i32);
}
```

The primary key is a `oneof` — `primary_key: Option<table_change::PrimaryKey>` — so match the variant rather than reaching for a `pk` field (there isn't one).

Verify rows land end to end (short ranges need a small flush interval; the default is 1000 blocks):

```bash
substreams sink postgres ./pkg.spkg -s 18000000 -t +100 --dsn "$DSN" --batch-block-flush-interval=1
psql "postgresql://user:pass@localhost:5432/db" -c "SELECT COUNT(*) FROM transfers;"
```

> The `psql` CLI needs `postgresql://`; the sink needs `psql://` or `postgres://` and **rejects**
> `postgresql://`. The two tools take different schemes for the same database — keep both handy.
