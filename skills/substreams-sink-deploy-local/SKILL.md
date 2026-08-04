---
name: substreams-sink-deploy-local
description: Use when the user wants to run or operate a Substreams sink themselves — take a built .spkg and pipe its data into a destination (Postgres, ClickHouse, files, PubSub, webhook) on infrastructure they manage. Covers sink CLIs, schema setup, cursor management, reorg handling, and production operations. For StreamingFast-hosted sinks (managed via the Portal API) use substreams-hosted-sink instead. Distinct from substreams-sql (building the db_out Rust module) and substreams-sink (SDK-level app integration).
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.5.0
  author: StreamingFast
  documentation: https://docs.substreams.dev/how-to-guides/sinks
---

# Substreams Sink Deployment Expert (Self-Managed)

End-to-end guide for taking a working Substreams package and running its sink yourself — on your own machine, server, or container. For a StreamingFast-**hosted** sink (no infrastructure to manage, driven entirely through the Portal API), use the `substreams-hosted-sink` skill instead.

## When to Use This Skill

Use this skill when the user says any of:
- "deploy my substreams" (and runs it themselves)
- "I want to send data to Postgres / ClickHouse"
- "stream my Substreams to S3 / GCS / files"
- "publish to PubSub / a webhook"
- "set up the SQL sink (`substreams sink postgres` / `clickhouse`) / -files / -pubsub"

**Do NOT use this skill** when the user is:
- Wanting StreamingFast to host & run the sink for them → use `substreams-hosted-sink`
- Writing the Substreams Rust code → use `substreams-dev`
- Picking field names for `db_out` / `DatabaseChanges` → use `substreams-sql`
- Consuming Substreams in a Go/JS/Rust app → use `substreams-sink`

## Decision Tree: Which Sink?

```
Where does the data need to land?

├── SQL database queries (analytics, joins, BI)
│   └── Postgres / ClickHouse  →  substreams sink postgres|clickhouse  (built into the substreams CLI)
│
├── Object storage / data lake (S3, GCS, local FS)
│   └── CSV / Parquet bulk files →  substreams-sink-files   (community; or built-in protojson)
│
├── Real-time event bus
│   ├── Google Cloud PubSub      →  substreams-sink-pubsub  (proto: sf.substreams.sink.pubsub.v1.Publish)
│   └── HTTP webhook             →  substreams sink webhook (proto: any)
│
├── Subgraph (The Graph network)
│   └── graph-node                 →  Substreams-powered Subgraph (proto: EntityChanges)
│
├── Custom application code
│   └── Go / JS / Rust SDK       →  see substreams-sink skill
│
└── Just JSONL output (testing)
    └── substreams sink protojson  (built-in, zero install)
```

**SQL vs Files vs Stream SDK:**
- **SQL** → analytical queries, joins, BI. ~5-50K rows/sec.
- **Files** (CSV/Parquet) → archival, batch ETL, training data. Cheapest at scale.
- **Stream SDK** → existing app needs each event in process. Highest control.

## Architecture: Build Side vs Run Side

```
┌────────────────────────────────────┐    ┌──────────────────────────────────┐
│  BUILD SIDE (Rust / .spkg)         │    │  RUN SIDE (sink binary)          │
│  - Substreams package (.spkg)      │───▶│  - substreams-sink-<x> binary    │
│  - Output module emits the         │    │  - Reads from Substreams endpoint│
│    proto type the sink expects     │    │  - Writes to destination         │
│  - Manifest sink: section (SQL)    │    │  - Manages cursor + reorgs       │
└────────────────────────────────────┘    └──────────────────────────────────┘
        ↑                                          ↑
        │ substreams-dev / substreams-sql          │ THIS skill
```

**Key rule:** the sink binary refuses to run if the output module emits the wrong proto type.

| Sink                        | Output module proto type                                   |
|-----------------------------|------------------------------------------------------------|
| `substreams sink postgres`/`clickhouse` (CDC) | `proto:sf.substreams.sink.database.v1.DatabaseChanges` |
| `substreams sink postgres`/`clickhouse` (from-proto) | your annotated domain proto (insert-only)  |
| `substreams-sink-pubsub`    | `proto:sf.substreams.sink.pubsub.v1.Publish`               |
| `substreams-sink-files`     | depends on `--encoder` — see Files section (`lines` requires `sf.substreams.sink.files.v1.Lines`) |
| `substreams sink webhook`   | any (delivered as JSON)                                    |
| Subgraph (graph-node)       | `proto:sf.substreams.sink.entity.v1.EntityChanges`         |
| `substreams sink protojson` | any (proto-as-JSON lines)                                  |

Wrong output type → redirect to `substreams-sql` (`db_out`) or `substreams-dev` before running the sink.

---

## Sink: SQL (Postgres / ClickHouse)

### Install

The SQL sink is **built into the `substreams` CLI** as of **v1.20.2** — there is no separate binary:

```bash
brew install streamingfast/tap/substreams                     # preferred
# or: https://github.com/streamingfast/substreams/releases
# or: docker pull ghcr.io/streamingfast/substreams

substreams sink postgres --help
substreams sink clickhouse --help
```

> **Migrating from the standalone binary?** `substreams-sink-sql` is deprecated and folded into the CLI. Existing databases keep working — cursor tables and schemas are unchanged, so the CLI resumes where the binary left off. Full command/flag mapping: [migration guide](https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/migration.md). On the old binary the conventions differ (positional DSN, positional `<start>:<stop>` range, `--clickhouse-*` flags, `--metrics-listen-addr`).

### Command tree

```
substreams sink postgres   <manifest> [<module>]     # runs the sink — there is NO `run` subcommand
substreams sink postgres   setup <manifest>
substreams sink postgres   generate-csv | inject-csv | tools cursor {read,write,delete}

substreams sink clickhouse <manifest> [<module>]
substreams sink clickhouse setup <manifest>
substreams sink clickhouse tools cursor {read,write,delete}
```

The engine is part of the command name and must match the DSN scheme. `generate-csv`/`inject-csv` are **PostgreSQL-only**. There is no `create-user` command — create the database user yourself.

### Two mapping modes (auto-detected)

The engine command and `setup` both detect the mode from the **output module's proto type** — there is no `from-proto` subcommand any more:

| Mode | Triggered by | Engine | Module output | Ops |
|------|--------------|--------|---------------|-----|
| **Database Changes (CDC)** | output type is `DatabaseChanges` | **PostgreSQL** | `DatabaseChanges` | INSERT/UPDATE/UPSERT/DELETE |
| **From-proto (relational)** | any other output type | Postgres **or ClickHouse** | your annotated proto | insert-only |

For ClickHouse, prefer **from-proto** (see `substreams-sql` skill). CDC against ClickHouse is not the guided path here.

Build-side details (Rust, annotations, ClickHouse ORDER BY rules) → **`substreams-sql`**.

### Required layout (Database Changes → Postgres)

```
my-substreams/
├── substreams.yaml   # modules + sink: section with schema path
├── schema.sql        # YOUR CREATE TABLE statements (hand-written)
└── my-substreams-v0.1.0.spkg
```

The package **must** declare a `sink:` block so `setup` and the engine command know the module and schema:

```yaml
sink:
  module: db_out
  type: sf.substreams.sink.sql.service.v1.Service   # sf.substreams.sink.sql.v1.Service still works but is deprecated
  config:
    schema: ./schema.sql
    engine: postgres
```

`setup` **always** resolves the module from `sink: module:` — it takes only a manifest argument, with no positional module override. Without that block it fails with `sink module is required in sink config`. The engine command infers the same way, but there you *can* pass the module as a second positional argument.

`setup` works in **both** modes, detecting which from the output module type:

- **Database Changes**: creates the bookkeeping tables (`cursors`, `substreams_history`) **and** applies your `schema.sql`.
- **From-proto**: derives the schema from the module's output proto, creates the database schema and tables, then exits. Idempotent — safe to re-run. (The engine command does this on startup too, so `setup` is optional here; run it when you want the DDL applied ahead of time.)

There is **no** `generate` command for schema templates — write `schema.sql` yourself (or copy from a known-good package / the `substreams-sql` skill patterns). Do not confuse with `generate-csv` (bulk CSV dump for high-throughput inject).

### DSN

The DSN is a `--dsn` flag or the `SUBSTREAMS_SINK_DSN` env var — **not** a positional argument. Its scheme must match the engine in the command name, and these are the accepted schemes ( **`postgresql://` is rejected** ):

```bash
# Postgres — use psql:// or postgres:// only
psql://user:pass@host:5432/dbname?sslmode=disable
postgres://user:pass@host:5432/dbname?sslmode=disable

# ClickHouse — native TCP 9000/9440, NOT HTTP 8123
clickhouse://default:pass@host:9000/dbname
# ClickHouse Cloud: clickhouse://default:pass@host:9440/default?secure=true
```

Optional Postgres isolation: `?schemaName=ethereum` (sink-specific; multi-pipeline on one DB).

> **Note:** Parquet/CSV lake output is **not** a SQL DSN — use `substreams-sink-files` or `substreams sink protojson`.
>
> An invalid scheme errors with `allowed schemes: [psql,postgres,clickhouse,parquet]`. **Ignore the `parquet` entry** — it is an unimplemented leftover, not a sink target. Passing `parquet://` is rejected up front anyway, since the DSN scheme has to match the engine in the command name (`DSN scheme "parquet" does not match command "postgres"`).

### Setup → Run (Database Changes / Postgres)

```bash
# CLI: substreams sink postgres <manifest> [<module>] [flags]   — no `run`, no positional DSN
# Module defaults to the package's sink: section; pass it positionally only to override.
# -e/--endpoint optional if the .spkg embeds network/endpoint; else required.

export SUBSTREAMS_API_KEY=server_xxx   # or SUBSTREAMS_API_TOKEN for JWT/legacy
export SUBSTREAMS_SINK_DSN="psql://user:pass@localhost:5432/mydb?sslmode=disable"

substreams sink postgres setup ./my-substreams.spkg

substreams sink postgres ./my-substreams.spkg \
    -s 12000000 -t +1000000 \
    -e "https://mainnet.eth.streamingfast.io:443"

# Short test ranges: force flush every block (default batch flush is 1000 blocks)
substreams sink postgres ./pkg.spkg -s 18000000 -t +100 -e "$EP" \
    --batch-block-flush-interval=1
```

Block range is `-s/--start-block` and `-t/--stop-block`, exactly like `substreams run` (`-t +N` = N blocks past start; omit `-t` for live tail). The old positional `START:STOP` is gone — the only positional range left is `inject-csv`'s.

Auth is **data-plane** auth for the streaming endpoint — not the Portal admin API. A Portal token from `portal-api-jwt` does **not** authenticate this self-managed sink.

### From-proto path (Postgres or ClickHouse)

Same command — the mode follows the output module's proto type, and the schema is derived from protobuf `schema.*` annotations (no `schema.sql`):

```bash
substreams sink postgres ./substreams.yaml --dsn "$DSN"     # add a module name after the manifest to override sink: module:
# ClickHouse example:
substreams sink clickhouse ./substreams.yaml --dsn "clickhouse://default:@localhost:9000/default"
```

ClickHouse-only flags (de-prefixed from the old `--clickhouse-*`): `--cluster`, `--cursor-file-path`, `--sink-info-folder`, `--query-retry-count`, `--query-retry-sleep`.

### Cursor management

Cursor is written automatically (Postgres: `cursors` table for CDC, `_cursor_` for from-proto; ClickHouse from-proto: cursor file, `--cursor-file-path`, default `cursor.txt`). **Do not manage cursors by hand** for normal restarts.

To intentionally wipe cursor(s) and reprocess:

```bash
# There is no `undo` subcommand
substreams sink postgres tools cursor delete --all --dsn "$DSN"
# Or delete a single module hash:
# substreams sink postgres tools cursor delete <module_hash> --dsn "$DSN"
```

Also: `tools cursor read` / `tools cursor write` for operators. Wiping the cursor reprocesses from the original start block — clear the destination tables too or you get duplicate rows.

### Reorg handling

- **Postgres CDC**: real-time undo via `substreams_history`. Keep `--undo-buffer-size=0` (default) for DB reorg handling.
- **ClickHouse / buffered path**: non-zero `--undo-buffer-size` delays writes until N confirmations (no in-DB undo).
- Never-uncommitted data (accounting): Postgres CDC **or** `--final-blocks-only`.

### Want StreamingFast to host this SQL sink?

Use `substreams-hosted-sink` (Portal API) instead of running the binary.

---

## Sink: Files (CSV / Parquet → S3, GCS, local)

Community-maintained binary ([substreams-sink-files](https://github.com/streamingfast/substreams-sink-files)). Prefer official `substreams sink protojson` when JSONL is enough.

### Install

```bash
brew install streamingfast/tap/substreams-sink-files
# or release binary from GitHub
```

### Output proto: depends on the encoder

There is **no `csv` or `jsonl` encoder.** The three real encoders (`--encoder`, default `parquet`):

| Encoder | Output module must be | Produces |
|---------|-----------------------|----------|
| `parquet` (default) | any protobuf message (parquet column options honored) | Parquet |
| `lines` | **`sf.substreams.sink.files.v1.Lines`** (list of strings) | any line format — JSONL, CSV, TSV |
| `protojson:.<field>[]` | any protobuf message | JSONL, one row per repeated entry |

CSV comes from the `lines` encoder with the **module** emitting CSV lines — not from a CSV encoder. The `protojson:` expression supports only the single form `.<repeated_field_name>[]`.

### Run (typical)

Signature: `substreams-sink-files run <manifest> [<module>] [<output>] [flags]` — the endpoint is the `-e` **flag**, and the block range is `-s`/`-t`, **not** positional args.

```bash
substreams-sink-files run \
    ./my-substreams.spkg \
    map_transfers \
    -o s3://my-bucket/transfers/ \
    -e "$ENDPOINT" \
    -s 12000000 -t +1000000 \
    --encoder=parquet \
    --file-block-count=10000
```

Storage (`-o`, default `./output`): `s3://`, `gs://`, `file://`, local paths.

Cursor: `--state-store`, default **`./state.yaml`** — a local path by default, *not* written next to `-o`. Persist it or restarts resume from `-s`.

File sizing is `-c/--file-block-count` (default 10000 blocks per file). `--buffer-max-size` is the writer's **memory budget in bytes** (default 67108864 = 64 MiB) — raise it toward available RAM for throughput; lowering it hurts.

Verify flags with `substreams-sink-files run --help` if the installed version differs.

---

## Sink: PubSub

```bash
go install github.com/streamingfast/substreams-sink-pubsub/cmd/substreams-sink-pubsub@latest
# or clone repo and go install ./cmd/substreams-sink-pubsub
```

Required output: `proto:sf.substreams.sink.pubsub.v1.Publish`.

```bash
substreams-sink-pubsub sink \
    -e "$ENDPOINT" \
    --project my-gcp-project \
    ./my-substreams.spkg \
    map_publish \
    my-topic-name
```

Credentials: `GOOGLE_APPLICATION_CREDENTIALS` or `gcloud auth application-default login`. Needs `pubsub.publisher` on the topic.

---

## Sink: Webhook (built into `substreams` CLI)

```bash
# Usage: substreams sink webhook <url> [<manifest> [<module_name>]] [flags]
substreams sink webhook \
    https://my-app.example.com/webhook \
    ./my-substreams.spkg \
    map_events \
    -e "$ENDPOINT"
```

Each block output is POSTed as JSON. Endpoint must return 2xx (retries with backoff). Cursor defaults to **`./state.cursor`** (`--state-file`). Mount/persist that path in Docker.

**Not for high volume** — prefer PubSub or SQL past ~100 events/sec.

---

## Sink: ProtoJSON (testing / debugging)

```bash
# Usage: substreams sink protojson <manifest> [<module>] [flags]
substreams sink protojson \
    ./my-substreams.spkg \
    map_my_module \
    -e "$ENDPOINT" \
    -o ./output_dir/ \
    -s 12000000 -t +100
```

`-o` supports local paths, `gs://`, `s3://`. Cursor default: `./state.yaml` when used.

---

## Sink: Subgraph (The Graph)

Deploy via `graph deploy`, not a substreams sink binary. Output module must be `proto:sf.substreams.sink.entity.v1.EntityChanges`. Patterns → `substreams-dev`.

---

## Self-Managed vs Hosted

| You want | Pick |
|----------|------|
| Zero ops, StreamingFast runs SQL | **Hosted** → `substreams-hosted-sink` |
| Free / self-managed Postgres/CH | This skill (`substreams sink postgres`/`clickhouse`) |
| Subgraph on The Graph | Substreams-powered Subgraph |
| Files / data lake | `substreams-sink-files` or protojson |
| Events in your app process | Stream SDK (`substreams-sink`) |

---

## Common Pitfalls

### 1. Wrong output proto type → sink refuses to start

Module output must match the sink table above. For SQL CDC use `DatabaseChanges`, not your domain proto.

### 2. Missing or incomplete `schema.sql` / `sink:` config

`setup` applies **your** `schema.sql` from the manifest sink config and creates internal tables. No auto-generation of domain tables. Hand-write `CREATE TABLE IF NOT EXISTS ...` aligned with `db_out` PK columns.

### 3. Long-running container restarts at block 0

Cursor not persisted. SQL: lives in DB. Files: `./state.yaml` (`--state-store`). Webhook: `./state.cursor` (`--state-file`). Protojson: `./state.yaml` (`--state-file`). Every non-SQL default is a **local relative path** — in a container it dies with the container. Mount volumes or point the flag at durable storage.

### 4. ClickHouse / buffered path sees no rows for ~N blocks

Undo buffer / finality delay. Tune `--undo-buffer-size` or use `--final-blocks-only` when appropriate.

### 5. Auth required

Default env vars: `SUBSTREAMS_API_KEY` (API keys) and/or `SUBSTREAMS_API_TOKEN` (JWT/legacy). Override names with `--api-key-envvar` / `--api-token-envvar`. Portal tokens are not valid here.

### 6. Short range, zero rows — batch flush default is 1000

```bash
substreams sink postgres ./pkg.spkg -s 18000000 -t +100 -e "$EP" --dsn "$DSN" \
    --batch-block-flush-interval=1
```

Production: leave default `1000` (or higher for Solana-scale). Lower only for tests / low volume.

### 7. Composite PK wire format mismatch (CDC)

| `schema.sql` | Rust `db_out` |
|--------------|---------------|
| `PRIMARY KEY (id)` | `tables.create_row("t", &id)` |
| `PRIMARY KEY (tx_hash, log_index)` | `tables.create_row("t", [("tx_hash", &h), ("log_index", li)])` |

Mismatch → rewrite Rust or use a single synthetic PK. Build patterns → `substreams-sql`.

### 8. Module hash mismatch after code/schema change

```bash
substreams sink postgres ... --on-module-hash-mismatch=warn   # or ignore
```

Neither resets the cursor — you mix old and new data. For a clean restart: wipe destination/cursor and re-run from the intended start block.

### 9. PubSub "permission denied"

Service account needs `pubsub.publisher`. Check ADC / `GOOGLE_APPLICATION_CREDENTIALS`.

### 10. Invented CLI — commands and flags that do not exist

Muscle memory from the old standalone binary produces most of these:

- **No `run` subcommand.** `substreams sink postgres <manifest>` runs the sink itself. Typing `run` is caught with an explicit error, but do not write it into scripts or docs.
- **No `from-proto` subcommand** — the mode is auto-detected from the output module type.
- **No `create-user`** — removed; create the database user directly in your database.
- **No** `generate` — write `schema.sql` yourself (`generate-csv` is a different, real command).
- **No** `undo` — use `tools cursor delete`.
- **No positional DSN and no positional block range** — `--dsn`/`SUBSTREAMS_SINK_DSN` and `-s`/`-t`. (`inject-csv` keeps its positional `<start>:<stop>` file range.)
- **No** `--metrics-listen-addr` — it is `--prometheus-addr` now (same `localhost:9102` default).
- **No** `--workers` — do not invent parallel-worker flags. For provider-side parallelism the real knob is the header `-H X-Substreams-Parallel-Workers=<n>` (the only other valid header is `X-substreams-acknowledge-non-deterministic`); otherwise use sequential backfill ranges.
- **No** `csv` / `jsonl` encoder on `substreams-sink-files` — only `parquet`, `lines`, `protojson:.<field>[]`.
- **No** `parquet://` sink target. `parquet` appears in the DSN parser's allowed scheme list, but nothing implements it, and the engine/DSN scheme check rejects it before anything runs. Use `substreams-sink-files --encoder=parquet`.

---

## Production Patterns

### Backfill then live

```bash
# Bounded historical range
substreams sink postgres ./pkg.spkg -s 12000000 -t 18000000 -e "$EP" --dsn "$DSN"

# Live tail from handoff block (omit -t)
substreams sink postgres ./pkg.spkg -s 18000000 -e "$EP" --dsn "$DSN"
```

High-volume Postgres inject: `generate-csv` → `inject-csv` → run the sink. Validate `cursors` with `tools cursor read` after inject.

### Monitoring

Prometheus: `--prometheus-addr` (default **`localhost:9102`**) — this replaced the old `--metrics-listen-addr`. Bind `0.0.0.0:9102` to scrape from outside a container. Scrape the process and alert on stall (last processed block not advancing) and elevated error/undo rates. Prefer live metric names from `/metrics` over hard-coded lists — they vary by version. pprof is opt-in via `--pprof-listen-addr`.

### Reorg-safe accounting

Balances/totals: Postgres + `db_out` UPSERT. From-proto is insert-only. When in doubt, `--final-blocks-only`.

### Restart safety

Kill mid-batch and restart: cursor resumes last committed batch → no duplicate rows (atomic per-batch commit). Do not add ad-hoc dedup.

---

## Quick Reference: Full Example (SQL CDC → Postgres)

```bash
export SUBSTREAMS_API_KEY=server_xxx
export SUBSTREAMS_SINK_DSN="psql://user:pass@localhost:5432/mydb?sslmode=disable"
export PSQL_DSN="postgresql://user:pass@localhost:5432/mydb?sslmode=disable"  # psql client only

# 0. Package with db_out + sink: { module, schema: ./schema.sql }  (substreams-sql skill)
# 1. Apply schema + system tables
substreams sink postgres setup ./erc20.spkg

# 2. Run the sink (no `run` subcommand)
substreams sink postgres ./erc20.spkg -s 12000000 -t +10000 \
    -e "https://mainnet.eth.streamingfast.io:443" \
    --prometheus-addr=localhost:9102

# 3. Query
psql "$PSQL_DSN" -c "SELECT count(*) FROM erc20_transfers"
```

---

## Resources

- Sinks overview: https://docs.substreams.dev/how-to-guides/sinks
- SQL sink (built into the CLI): https://github.com/streamingfast/substreams
- Migrating off the standalone SQL sink: https://github.com/streamingfast/substreams/blob/develop/docs/how-to-guides/sinks/sql/migration.md
- Files sink: https://github.com/streamingfast/substreams-sink-files
- PubSub sink: https://github.com/streamingfast/substreams-sink-pubsub
- The Graph (subgraphs): https://thegraph.com/docs/en/cookbook/substreams-powered-subgraphs/
- Hosted path: `substreams-hosted-sink` skill · Build SQL modules: `substreams-sql` skill
