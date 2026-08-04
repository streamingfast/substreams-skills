# T7.1 — Deploy a SQL Sink to Postgres

**Skill exercised:** `substreams-sink-deploy-local`
**Model:** claude-sonnet-4-6
**Result:** PASS — sink installed, schema applied, **537/537 rows match golden** · 5 trials, all PASS once known gotchas were patched into the skill

> **Historical note:** this eval ran against the standalone `substreams-sink-sql` v4.13.1 binary, which has since been folded into the `substreams` CLI (v1.20.2+) and deprecated. Commands below are preserved as run. Today the equivalents are `substreams sink postgres setup <manifest> --dsn "$DSN"` and `substreams sink postgres <manifest> -s 18000000 -t +100 --dsn "$DSN" --batch-block-flush-interval=1` — note there is no `run` subcommand. See the [migration guide](https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/migration.md).

## Goal

Take a built `.spkg` (the T2.3 SQL sink) and deploy it end-to-end:

1. Stand up local Postgres (Docker)
2. Install / locate `substreams-sink-sql` binary
3. Generate `schema.sql`, create database tables
4. Run the sink against blocks `18000000:+100`
5. Verify with `SELECT count(*) FROM usdc_transfers`

This is operational, not Rust-coding — the skill covers CLI arg order, DSN scheme, schema setup sequencing, and the batch-flush flag.

## Prompt

Prompt supplied the `.spkg` path, Postgres credentials, endpoint URL, and expected verify query. Not reproduced here.

## What the skill provided

The skill's "Common Pitfalls" section covered all three issues that surfaced during the trial:

1. **DSN scheme** — `substreams-sink-sql` v4.13.1 rejects `postgresql://`. Allowed: `psql://`, `postgres://`, `clickhouse://`, `parquet://`. Skill quick-reference example was patched to use `psql://`.
2. **Composite-PK schema mismatch** *(fixed in T2.3)* — Early trials had a mismatch between the embedded schema PK and the Rust `db_out` PK. T2.3 was subsequently fixed to use composite PKs consistently — no workaround needed.
3. **`--batch-block-flush-interval=1`** — without it (default 1000), a 100-block range completes without flushing to DB and the verify query returns 0 rows.

## Files

- [`deployment-notes.md`](deployment-notes.md) — agent's actual deployment writeup, including the issues encountered + fix sequence

## Reproduce

```bash
docker run -d -p 5436:5432 -e POSTGRES_PASSWORD=secret postgres:15
go install github.com/streamingfast/substreams-sink-sql/cmd/substreams-sink-sql@v4.13.1

# Build the .spkg from the T2.3 example first
substreams-sink-sql setup "psql://postgres:secret@localhost:5436/postgres?sslmode=disable" usdc-sql-sink-v0.1.0.spkg

substreams-sink-sql run \
  "psql://postgres:secret@localhost:5436/postgres?sslmode=disable" \
  usdc-sql-sink-v0.1.0.spkg \
  18000000:+100 \
  --batch-block-flush-interval=1
```
