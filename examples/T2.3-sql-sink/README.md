# T2.3 — Postgres SQL Sink (Ethereum)

**Skills exercised:** `substreams-dev`, `substreams-sql`
**Model:** claude-sonnet-4-6
**Result:** Build OK · Run OK · Correctness 100% · 2 trials, both 6/6

## Goal

Take the T1.2 USDC transfers module and add a `db_out` map that emits `sf.substreams.sink.database.v1.DatabaseChanges`, plus a `schema.sql`. Composite primary key on `(tx_hash, log_index)`. Wire it up so `substreams-sink-sql` can consume it.

> **Historical note:** this eval predates the fold-in of `substreams-sink-sql` into the `substreams` CLI (v1.20.2+). The build side below — `db_out`, `schema.sql`, the `sink:` block — is unchanged; only the consuming CLI moved, to `substreams sink postgres`. See [T7.1](../T7.1-sink-sql-deploy/) and the [migration guide](https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/migration.md).

## Prompt

> I have a Substreams module that produces USDC Transfer events. I want to persist every transfer as a row in Postgres using `substreams-sink-sql`.
>
> Please:
> 1. Add a `db_out` map module that outputs `sf.substreams.sink.database.v1.DatabaseChanges`.
> 2. Write a `schema.sql` with a `usdc_transfers` table — one row per transfer.
> 3. Wire it all up in `substreams.yaml` so `substreams-sink-sql` can consume it.
>
> Use `(tx_hash, log_index)` as the primary key.
>
> The upstream module is `map_usdc_transfers` which outputs `usdc.transfers.v1.UsdcTransfers`.

## What the skill provided

- `substreams-database-change` crate FQN path (`substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges`)
- `DatabaseChanges` builder pattern (`tables::Tables` + `create_row` / `set`)
- Composite-PK array syntax matching `schema.sql` `PRIMARY KEY (tx_hash, log_index)`
- `substreams.yaml` `db_out` wiring + **v4.0.0** database-changes spkg (matches crate `= "4"`)

## Files

- [`substreams.yaml`](substreams.yaml)
- [`Cargo.toml`](Cargo.toml)
- [`schema.sql`](schema.sql)
- [`proto/usdc_transfers.proto`](proto/usdc_transfers.proto)
- [`src/lib.rs`](src/lib.rs)

## Reproduce

```bash
substreams build
substreams run ./substreams.yaml db_out -s 18000000 -t +100 -o jsonl
```

To deploy the resulting `.spkg` to a real Postgres, see [T7.1](../T7.1-sink-sql-deploy/).
