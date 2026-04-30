---
name: substreams-sink-deploy
description: Use when the user wants to run, deploy, or operate a Substreams sink — take a built .spkg and pipe its data into a destination (Postgres, ClickHouse, files, PubSub, webhook). Covers sink CLIs, schema setup, cursor management, reorg handling, and production operations. Distinct from substreams-sql (building the db_out Rust module) and substreams-sink (SDK-level app integration).
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.1.0
  author: StreamingFast
  documentation: https://docs.substreams.dev/how-to-guides/sinks
---

# Substreams Sink Deployment Expert

End-to-end guide for taking a working Substreams package and getting its data into a destination — local, hosted, or managed.

## When to Use This Skill

Use this skill when the user says any of:
- "deploy my substreams"
- "I want to send data to Postgres / ClickHouse"
- "stream my Substreams to S3 / GCS / files"
- "publish to PubSub / a webhook"
- "deploy to a hosted sink"
- "set up substreams-sink-sql / -files / -pubsub"

**Do NOT use this skill** when the user is:
- Writing the Substreams Rust code → use `substreams-dev`
- Picking field names for `db_out` / `DatabaseChanges` → use `substreams-sql`
- Consuming Substreams in a Go/JS/Rust app → use `substreams-sink`

## Decision Tree: Which Sink?

```
Where does the data need to land?

├── SQL database queries (analytics, joins, BI)
│   └── Postgres / ClickHouse  →  substreams-sink-sql        (proto: DatabaseChanges)
│
├── Object storage / data lake (S3, GCS, local FS)
│   └── CSV / Parquet bulk files →  substreams-sink-files   (proto: any custom)
│
├── Real-time event bus
│   ├── Google Cloud PubSub      →  substreams-sink-pubsub  (proto: sf.substreams.sink.pubsub.v1.Publish)
│   └── HTTP webhook             →  substreams sink webhook (proto: any)
│
├── Subgraph (The Graph network)
│   └── graph-node                 →  Substreams-powered Subgraph (proto: sf.substreams.sink.entity.v1.EntityChanges)
│
├── Custom application code
│   └── Go / JS / Rust SDK       →  see substreams-sink skill (no binary, app-level)
│
└── Just JSONL output (typically files)
    └── substreams sink protojson  (built-in, zero install — for testing)
```

**Picking SQL vs Files vs Stream SDK** — most common decision:
- **SQL** → you'll run analytical queries, need joins, need ad-hoc BI. ~5-50K rows/sec.
- **Files** (CSV/Parquet) → archival, batch ETL into a warehouse, training data. Cheapest at scale.
- **Stream SDK** → you have an existing app and need each event passed to your code. Highest control, most code to write.

## Architecture: Build Side vs Run Side

Every sink has two halves. Skill tasks usually mix them up.

```
┌────────────────────────────────────┐    ┌──────────────────────────────────┐
│  BUILD SIDE (Rust / .spkg)         │    │  RUN SIDE (sink binary)          │
│                                    │    │                                  │
│  - Substreams package (.spkg)      │───▶│  - substreams-sink-<x> binary    │
│  - Output module emits the         │    │  - Reads from Substreams endpoint│
│    proto type the sink expects     │    │  - Writes to destination         │
│  - One module per sink type        │    │  - Manages cursor + reorgs       │
└────────────────────────────────────┘    └──────────────────────────────────┘
        ↑                                          ↑
        │ owned by substreams-dev /                │ owned by THIS skill
        │ substreams-sql skills                    │
```

**Key rule:** the sink binary will refuse to run if your output module emits the wrong proto type. Each sink expects a specific message.

| Sink                      | Output module proto type                                       |
|---------------------------|----------------------------------------------------------------|
| `substreams-sink-sql`     | `proto:sf.substreams.sink.database.v1.DatabaseChanges`         |
| `substreams-sink-pubsub`  | `proto:sf.substreams.sink.pubsub.v1.Publish`                   |
| `substreams-sink-files`   | any user-defined proto (you choose; the sink streams it raw)   |
| `substreams sink webhook` | any user-defined proto (delivered as JSON to the URL)          |
| Subgraph (graph-node)     | `proto:sf.substreams.sink.entity.v1.EntityChanges`             |
| `substreams sink protojson` | any (writes proto-as-JSON lines)                             |

If the user has the wrong output type, redirect them to the `substreams-sql` skill (for `db_out`) or the `substreams-dev` skill (for `EntityChanges` / custom protos) BEFORE running the sink.

---

## Sink: SQL (Postgres / ClickHouse)

### Install

```bash
# Binary release (preferred):
brew install streamingfast/tap/substreams-sink-sql            # macOS
# or download from: https://github.com/streamingfast/substreams-sink-sql/releases

# From source:
go install github.com/streamingfast/substreams-sink-sql/cmd/substreams-sink-sql@latest
```

### Required files

```
my-substreams/
├── substreams.yaml          # has a db_out module (DatabaseChanges output)
├── my-substreams-v0.1.0.spkg
└── schema.sql               # your CREATE TABLE statements
```

Your `schema.sql` contains your application tables. The `setup` command creates the sink's internal bookkeeping tables (`cursors`, `substreams_history`) automatically — you don't need to define them. Use `generate` to get a starter template with your domain tables pre-filled:

```bash
substreams-sink-sql generate <DSN> ./my-substreams.spkg <module_name>
```

### DSN format

The sink accepts these schemes only — `postgresql://` is **rejected** by v4.x:

```bash
# Postgres
psql://user:pass@host:5432/dbname?sslmode=disable
postgres://user:pass@host:5432/dbname?sslmode=disable     # alias

# ClickHouse
clickhouse://default:pass@host:9000/dbname
```

> **Note:** `parquet://` is NOT a `substreams-sink-sql` scheme — Parquet output uses `substreams-sink-files` with `--encoder=parquet`. See the Files sink section below.

### Setup → Run flow

```bash
# 1. Create DB schema (one-time)
substreams-sink-sql setup "$DSN" ./my-substreams.spkg

# 2. Run the sink (long-running process)
#    CLI signature: substreams-sink-sql run <DSN> <package.spkg> [<module_name>] [<block_range>] [-e <endpoint>]
#    -e/--endpoint is optional if the .spkg embeds a network/endpoint; required otherwise.
#    Module name is auto-inferred from the .spkg if a single sink module exists.
substreams-sink-sql run "$DSN" \
    ./my-substreams.spkg \
    "12000000:+1000000" \
    -e "https://mainnet.eth.streamingfast.io:443"
# Minimal (endpoint auto-derived from .spkg network config):
# substreams-sink-sql run "$DSN" ./my-substreams.spkg "12000000:+1000000"
```

Auth required: set `SUBSTREAMS_API_KEY` (new accounts) or `SUBSTREAMS_API_TOKEN` (JWT/legacy accounts). See pitfall #5 for details.

> **CLI arg order matters and is non-obvious.** Endpoint is a `-e/--endpoint` flag, NOT positional. The module name is positional but optional — only required if your `.spkg` has multiple sink-compatible output modules (rare). Block range syntax: `START:STOP` or `START:+N` for N blocks forward, or omit STOP for open-ended live tailing.

### Two schema mapping modes

The sink supports two ways to map proto → SQL:

1. **`db_out` module** (recommended). Your Substreams emits `DatabaseChanges` directly. Full control over INSERT / UPDATE / UPSERT. See `substreams-sql` skill for proto setup.
2. **Relational mappings ("from-proto")**. Sink derives SQL ops from your output proto using buf annotations on field tags. Simpler for read-only mirrors; can't express updates/deletes cleanly. Skip unless asked.

If you use `db_out`, the proto type MUST be:
```
output:
  type: proto:sf.substreams.sink.database.v1.DatabaseChanges
```

### Cursor management (zero config — automatic)

The sink writes the latest processed cursor to the `cursors` table on every batch. On restart, it picks up from there. **Do not manage cursors manually.** If you delete the row, the sink will re-process from the original `start_block`.

To intentionally restart from scratch:
```bash
substreams-sink-sql undo "$DSN" ./my-substreams.spkg --all
```

### Reorg handling

- **Postgres**: reorgs handled in real-time. Sink uses `substreams_history` to roll back forks.
- **ClickHouse**: reorgs supported with delay (waits N blocks for finality before writing). Set the buffer with `--undo-buffer-size`.

For tasks that must NEVER see uncommitted data (e.g. accounting, balances), use Postgres OR run with `--final-blocks-only`.

### Hosted SQL sink

> Hosted SQL sink is not yet released. Check https://docs.substreams.dev/how-to-guides/sinks for availability updates.

---

## Sink: Files (CSV / Parquet → S3, GCS, local)

### When to use

You need bulk data on object storage for downstream ETL (Snowflake, BigQuery, dbt, Athena). NOT for real-time application use — files are batched on a configurable interval.

### Install

```bash
go install github.com/streamingfast/substreams-sink-files/cmd/substreams-sink-files@latest
```

### Output proto: any (you choose)

Unlike SQL, the files sink doesn't mandate a proto shape. You design your output proto, the sink writes one row per repeated entry.

```protobuf
message Transfers {
  repeated Transfer transfers = 1;
}
message Transfer {
  string from = 1;
  string to = 2;
  string amount = 3;
  // ...
}
```

The sink walks `repeated` fields at the top level and emits one CSV/Parquet row per entry.

### Run

```bash
substreams-sink-files run \
    "$ENDPOINT" \
    ./my-substreams.spkg \
    map_transfers \
    s3://my-bucket/transfers/ \
    "12000000:+1000000" \
    --encoder=parquet \
    --buffer-max-size=200000   # rows per file
```

Encoders: `parquet` (recommended for warehouses), `csv`, `jsonl`.

Storage URLs: `s3://`, `gs://`, `file:///abs/path`.

### Cursor

Stored in the destination as a hidden `_cursor.json` next to the output files. Survives restarts.

---

## Sink: PubSub

### Install

```bash
go install github.com/streamingfast/substreams-sink-pubsub/cmd/substreams-sink-pubsub@latest
```

### Required output type

```
output:
  type: proto:sf.substreams.sink.pubsub.v1.Publish
```

The `Publish` proto holds the bytes to publish + topic attributes. Your map module wraps your domain proto into `Publish`.

```rust
use std::collections::HashMap;
use prost::Message as ProstMessage;
use substreams_sink_pubsub::pb::sf::substreams::sink::pubsub::v1::{Message, Publish};

#[substreams::handlers::map]
fn map_publish(block: Block) -> Result<Publish, Error> {
    let my_event = MyEvent { /* ... */ };
    let bytes = my_event.encode_to_vec();
    Ok(Publish {
        messages: vec![Message { data: bytes, attributes: HashMap::new() }],
    })
}
```

### Run

```bash
substreams-sink-pubsub sink \
    -e "$ENDPOINT" \
    --project my-gcp-project \
    ./my-substreams.spkg \
    map_publish \
    my-topic-name
```

GCP credentials via standard `GOOGLE_APPLICATION_CREDENTIALS` env var.

---

## Sink: Webhook (built into substreams CLI)

No separate binary needed. Use the built-in:

```bash
substreams sink webhook \
    -e "$ENDPOINT" \
    ./my-substreams.spkg \
    map_events \
    https://my-app.example.com/webhook
```

Each block's output is POSTed as JSON. Your endpoint must return 2xx; otherwise the sink retries with exponential backoff. Cursor is written to `./state.json` in the current working directory — persist or mount that path in Docker to resume across restarts.

**Don't use webhooks for high-volume data.** PubSub or SQL beats it past ~100 events/sec.

---

## Sink: ProtoJSON (testing / debugging)

Built-in, zero install. Just emits proto-as-JSONL files. Use this for local validation before plugging in a real sink:

```bash
substreams sink protojson \
    -e "$ENDPOINT" \
    ./my-substreams.spkg \
    map_my_module \
    ./output_dir/ \
    "12000000:+100"
```

---

## Sink: Subgraph (The Graph)

Substreams-powered subgraphs deploy via `graph deploy`, not via a substreams sink binary. Required output module:

```yaml
- name: graph_out
  kind: map
  inputs:
    - source: sf.substreams.type.v2.Block
  output:
    type: proto:sf.substreams.sink.entity.v1.EntityChanges
```

Then in `subgraph.yaml`:

```yaml
specVersion: 1.2.0
features:
  - nonFatalErrors
dataSources:
  - kind: substreams
    name: my-substreams
    network: mainnet
    source:
      package:
        moduleName: graph_out
        file: ./my-substreams-v0.1.0.spkg
    mapping:
      kind: substreams/graph-entities
      apiVersion: 0.0.7
schema:
  file: ./schema.graphql
```

Deploy: `graph deploy <subgraph-name>` (Studio or hosted).

For `EntityChanges` proto setup and module patterns → use the `substreams-dev` skill.

---

## Hosted vs Self-Hosted Decision

| You want                                           | Pick                                |
|----------------------------------------------------|-------------------------------------|
| Zero ops, managed Postgres (not yet released)      | StreamingFast hosted (sales@streamingfast.io) / Pinax |
| Free, you manage the box                           | Self-host `substreams-sink-sql`     |
| Subgraph on The Graph network                       | Substreams-powered Subgraph (decentralized) |
| Files into your data lake                          | Self-host `substreams-sink-files`   |
| Push events to your existing app                   | Stream SDK (`substreams-sink` skill) |

**Hosted sink is not yet released.** Track https://docs.substreams.dev/how-to-guides/sinks for availability updates.

---

## Common Pitfalls

### 1. Wrong output proto type → sink refuses to start

```
Error: module 'db_out' output type 'proto:my.types.v1.Events'
       does not match expected 'proto:sf.substreams.sink.database.v1.DatabaseChanges'
```

Fix: the substreams module's output proto must EXACTLY match what the sink expects. See table above. For SQL specifically, you must use `db_out` returning `DatabaseChanges` — not your domain proto. The `substreams-sql` skill covers building this module.

### 2. Hand-wrote `schema.sql` without running `generate` → missing domain tables

`setup` auto-creates the sink's internal tables (`cursors`, `substreams_history`), but it won't know your application tables. If you skipped `generate`, your app tables won't exist. Run:
```bash
substreams-sink-sql generate "$DSN" ./my-substreams.spkg <module>
```
then add your custom table definitions and re-run `setup`.

### 3. Long-running sink container restarts at block 0 every time

Cursor not persisted. For SQL sink: cursor lives in DB — not affected. For files sink: cursor lives in destination bucket — not affected if you re-point at the same URL. For webhook/protojson: `./state.json` must persist between runs (mount as volume in Docker).

### 4. `block.transactions()` works but ClickHouse sees no rows for ~50 blocks

Reorg buffer. Set `--undo-buffer-size=12` or smaller if you accept some rollback risk. Default holds writes until finality (~32 blocks on Eth, ~12 on Polygon, etc.).

### 5. `substreams-sink-sql` says "auth required" even with API key set

The sink reads `SUBSTREAMS_API_KEY` from env (this is the correct var name — NOT `SUBSTREAMS_API_TOKEN`). If you're using `--api-key-envvar=OTHER_VAR` make sure the env var name matches. JWT mode (older accounts) requires `SUBSTREAMS_API_TOKEN` instead.

### 6. Sink ran but no rows landed — `--batch-block-flush-interval` too large for short ranges

**Default is 1000.** If you process fewer than 1000 blocks (e.g. an eval task with `+100`), the sink may keep the batch in memory until either the interval is reached or the process exits cleanly and performs a final partial flush. Net result: you may see no rows during the run, and short test ranges can be misleading unless you lower the flush interval.

```bash
# Wrong (default 1000) — 100 blocks won't commit
substreams-sink-sql run "$DSN" ./pkg.spkg "18000000:+100" -e "$EP"

# Right — flush every block for short ranges
substreams-sink-sql run "$DSN" ./pkg.spkg "18000000:+100" -e "$EP" \
    --batch-block-flush-interval=1
```

**Production**: leave default `1000` (or set higher for very high-throughput chains like Solana). Tune down only for testing or very low-volume modules.

### 7. "substreams sent a single primary key, but our sql table has a composite primary key" — PK wire format mismatch

Sink v4+ enforces strict matching between `schema.sql` and the wire-format PK from your `db_out` Rust module:

| Your `schema.sql` declares                                  | Your Rust code MUST send                          |
|-------------------------------------------------------------|---------------------------------------------------|
| `PRIMARY KEY (id)` (single column)                          | `tables.create_row("t", &id)` — single string    |
| `PRIMARY KEY (tx_hash, log_index)` (composite, 2+ columns)  | `tables.create_row("t", [("tx_hash", &h), ("log_index", li)])` — slice of (col, val) tuples |

If you see this error: either rewrite the Rust to send a tuple slice, OR change the schema to a single-column synthetic PK (`id VARCHAR PRIMARY KEY`) and concatenate fields in Rust (`format!("{}-{}", tx_hash, log_index)`).

For build-side patterns → `substreams-sql` skill.

### 8. Module hash mismatch on restart after schema/code changes

You changed your `.spkg` and now the sink errors:
```
module hash mismatch: cursor was for hash X, current module is hash Y
```

The cursor pins a specific module-hash to detect drift. Two options to override:
```bash
# Log a warning but continue from the existing cursor (data may be inconsistent)
substreams-sink-sql run ... --on-module-hash-mismatch=warn

# Same as warn but silently — still continues from existing cursor, no reset
substreams-sink-sql run ... --on-module-hash-mismatch=ignore
```

Neither flag resets the cursor. Both continue from where the old module left off, mixing old and new data — a debugging nightmare if your schema or logic changed. For a clean restart, wipe the destination manually and re-run from the original start block.

### 9. PubSub "permission denied"

GCP service account needs `pubsub.publisher` on the topic. Check `gcloud auth application-default login` is set, OR `GOOGLE_APPLICATION_CREDENTIALS` points at a key file with the right role.

---

## Production Patterns

### Backfill then live

For initial load of millions of blocks then continuous tailing:

```bash
# 1. Backfill (bounded range, parallel-friendly with --workers=N)
substreams-sink-sql run "$DSN" ./pkg.spkg "12000000:18000000" -e "$EP" --workers=8

# 2. Live (start = stop block of backfill, omit stop for open-ended)
substreams-sink-sql run "$DSN" ./pkg.spkg "18000000:" -e "$EP" --workers=1
```

For files sink: same positional range pattern — `"12000000:18000000"` for backfill, `"18000000:"` for live.

### Monitoring

All sinks expose Prometheus metrics on `--metrics-listen-addr=:9100`:
- `substreams_sink_block_height` — last processed block
- `substreams_sink_blocks_per_second` — throughput
- `substreams_sink_undo_count` — reorgs handled

Alert on:
- block_height stalled for > N minutes
- undo_count rising fast (chain instability or node issue)

### Reorg-safe accounting

If your sink writes balances/totals, you MUST use Postgres + `db_out` with `UPSERT`. ClickHouse + relational-mappings can produce double-counted rows during reorgs. When in doubt, run with `--final-blocks-only` (sacrifices ~1 minute of latency for zero rollback risk).

### Restart safety

Kill -9 the sink mid-batch. Restart. Sink reads cursor → resumes at last committed block → no duplicate rows (atomic per-batch commit). This is by design; do not add your own dedup logic.

---

## Quick Reference: Full Example (SQL → Postgres)

```bash
# Prereqs
export SUBSTREAMS_API_KEY=server_xxx
export DSN="psql://user:pass@localhost:5432/mydb?sslmode=disable"       # sink DSN (psql:// or postgres://)
export PSQL_DSN="postgresql://user:pass@localhost:5432/mydb?sslmode=disable"  # for psql client

# 0. Get a working substreams package with a db_out module
#    (see substreams-sql skill for the build side)

# 1. Generate schema.sql template
substreams-sink-sql generate "$DSN" ./erc20.spkg db_out > schema.sql
# Edit schema.sql to add your domain tables (setup auto-creates the internal cursors/substreams_history tables)

# 2. Create DB tables
substreams-sink-sql setup "$DSN" ./erc20.spkg

# 3. Run sink (foreground; use systemd/docker for production)
substreams-sink-sql run "$DSN" \
    ./erc20.spkg \
    "12000000:+10000" \
    -e "https://mainnet.eth.streamingfast.io:443" \
    --metrics-listen-addr=:9100

# 4. Query
psql "$PSQL_DSN" -c "SELECT count(*) FROM erc20_transfers"
```

---

## Resources

- Sinks overview: https://docs.substreams.dev/how-to-guides/sinks
- SQL sink: https://github.com/streamingfast/substreams-sink-sql
- Files sink: https://github.com/streamingfast/substreams-sink-files
- PubSub sink: https://github.com/streamingfast/substreams-sink-pubsub
- The Graph (subgraphs): https://thegraph.com/docs/en/cookbook/substreams-powered-subgraphs/
