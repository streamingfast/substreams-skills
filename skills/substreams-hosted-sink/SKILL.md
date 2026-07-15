---
name: substreams-hosted-sink
description: >
  Use when the user wants StreamingFast to host and run a Substreams sink — deploy a .spkg
  to managed infrastructure (PostgreSQL or ClickHouse only) via the Portal API HostedService.
  HARD RULE: before any first-time hosted Deploy (or any "ready to deploy?" prompt), stop and
  offer a substreams run output-quality check as its own turn. For that check, give the user a
  ready-to-paste substreams run command only (stdout/jsonl) — never substreams-sink-sql, never
  a local ClickHouse/Postgres sync. Building the module is not enough. SF hosts only the sink
  runner; the user supplies the database. Self-managed sinks: substreams-sink-deploy-local;
  modules: substreams-sql / substreams-dev.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.12.1
  author: StreamingFast
  documentation: https://docs.substreams.dev/how-to-guides/sinks
---

# Substreams Hosted Sink

Deploy and operate a Substreams sink on **StreamingFast-hosted** infrastructure — entirely through the Portal API's `HostedService`. There is no binary to install and no server to manage: you hand StreamingFast a `.spkg` and the connection details for **your own** output database, and it runs, monitors, and scales the sink for you. Everything here is a Portal API call.

---

## ⛔ STOP — Output quality gate (read first, every deploy path)

**This is the #1 rule of this skill.** Agents routinely skip it and jump to “Ready to proceed to the hosted ClickHouse/Postgres deployment?” — that is a **skill violation**.

### What the quality check IS (and is NOT)

| ✅ Do this | ❌ Never do this for the quality gate |
|---|---|
| Give the user a **ready-to-paste `substreams run`** command | Run **`substreams-sink-sql`** / any sink binary |
| Inspect module output on **stdout** (`-o jsonl` or default UI) | **Sync / write** to local or remote **ClickHouse, Postgres, or any DB** |
| Same **`output_module`** + **network** as the future Deploy | Treat a local sink dry-run as the quality check |
| Short block range (e.g. `-t +100`) | Open DB credentials, Docker Compose DB, or `substreams-sink-deploy-local` |

**Purpose:** let the user eyeball protobuf/JSON **data quality** (fields, filters, empty vs expected) before hosted backfill writes into **their** production DB.  
**Not purpose:** prove the SQL sink path, create tables, or load a local database.

### What you must do

Before **any** of the following:

- Asking “ready to deploy / proceed to hosted deployment?”
- Collecting DB host for a **new** first Deploy
- Portal login for the purpose of Deploy
- `CreateDeployment` / secret page / `DeployDatabase` / `Deploy`
- Confirming a deploy plan that would create a new hosted sink

…you **must** already have set a session flag **`OUTPUT_TEST_STATUS`** to one of:

| Value | How it becomes set (only these ways) |
|---|---|
| `tested_ok` | You gave the user a **`substreams run`** command (and/or they ran it), they reviewed the **streamed module output** (not a DB), and they said it looks good / continue. |
| `user_verified` | You asked the mandatory offer turn below and the user **replied** that they already verified with `substreams run` (not your inference; not “I sank to local CH”). |
| `skipped` | You asked the mandatory offer turn and the user **replied** to skip the test. |

If `OUTPUT_TEST_STATUS` is **unset**, you **must not** talk about hosting as the next step. Your next user-facing message is **only** the offer turn — or, after they pick test-now, **only** the `substreams run` command + wait for their review.

### Forbidden until `OUTPUT_TEST_STATUS` is set

Do **not** say or imply any of:

- “Ready to proceed to the hosted ClickHouse/Postgres deployment?”
- “Shall we deploy the hosted sink now?”
- “Next we’ll deploy to StreamingFast hosted…”
- Jumping from engine choice / package publish / “spkg is ready” straight into deploy confirmation
- “Let’s test by sinking into local ClickHouse/Postgres first”

**Building, compiling, or publishing a package does not set the flag.**  
**A local sink sync does not set the flag** (wrong tool for this gate).  
**You deciding “looks fine” without the offer turn does not set the flag.**

### Exceptions (flag not required)

- Scale / pause / status / logs / events / undeploy / reset of an **existing** deployment  
- Reconfigure that does **not** change spkg or output module  

If reconfigure / redeploy changes **spkg URL, version, or `output_module`**, treat as a new deploy path → **reset the flag and re-offer the test**.

### Mandatory offer turn (copy this shape)

**One question, own turn, selectable choices, always include custom.** Do not batch with engine, DB host, or Portal login.

> Before any hosted deploy, check Substreams **data quality** with **`substreams run` only** (prints module output; does **not** write to a database). Hosted backfill will later write whatever this module emits into your DB.
>
> 1. **Give me the `substreams run` command** (recommended)  
> 2. **I already verified with `substreams run`** (tell me how)  
> 3. **Skip the test — deploy anyway**  
> 4. **Other / enter a custom answer**

Then **stop the turn** and wait. Do not add “and then we’ll deploy” as the main CTA in the same message unless status is already set.

| User picks | Set flag | Next action |
|---|---|---|
| **1 — Command** | leave **unset** until done | Fill in a concrete `substreams run` command (below), show it to the user, ask them to run it and confirm the printed data looks right. **Do not** start a sink. After they confirm OK → `OUTPUT_TEST_STATUS=tested_ok`. |
| **2 — Already verified** | `user_verified` | Proceed to engine/DB/Portal workflow. |
| **3 — Skip** | `skipped` | Warn once (bad data → wasted backfill / dirty DB), then proceed. |
| **4 — Other** | — | Clarify; re-offer 1–3 if still deploying. If they ask to “test with a local sink,” refuse for this gate and restate: quality check = `substreams run` only. |

### How to provide the test (when they pick 1)

**Agent job: hand the user the command.** Prefer they run it in their own terminal so they can scroll the output. Do **not** invoke sink tools. Optionally you may run the **same** `substreams run` yourself if the environment has data-plane auth — still **no sink**.

Fill every placeholder for their project:

```bash
# Data-plane auth — NOT the Portal Bearer token
# substreams auth   # once, if needed

substreams run ./substreams.yaml <output_module> \
  --network <network> \
  -s <start_block> -t +100 \
  -o jsonl
```

| Placeholder | Value |
|---|---|
| `./substreams.yaml` | Their manifest or a local `.spkg` path |
| `<output_module>` | Exact module that will be `execution_config.output_module` on Deploy |
| `<network>` | Same network id as Deploy (e.g. `solana`, `mainnet`) |
| `<start_block>` | A recent or known-good start for a short window |

**After they run it**, ask only: “Does the printed output look correct?” → yes sets `tested_ok`. If wrong, fix the package (`substreams-dev` / chain / `substreams-sql`) **before** hosted Deploy. Publish to a public URL remains a **later** step for Deploy — not part of this quality gate.

### Self-check before every deploy-related prompt

Mentally answer: **Is `OUTPUT_TEST_STATUS` set?**  
- **No** → only emit the offer turn, or the `substreams run` command + wait for review.  
- **Yes** → you may proceed to engine → DB → Portal → Deploy.

---

> ### ⚠️ StreamingFast does NOT host or provision any database
>
> The hosted offering is the **sink runner only**. StreamingFast never creates, hosts, or manages a PostgreSQL or ClickHouse database for you — there is **no** "StreamingFast-provisioned database" product. The output database is **always supplied by the user**, and the hosted sink connects to it **remotely over the network**.
>
> Consequences:
> - The user must bring a database that is **already running and reachable from StreamingFast's network** (public hostname/IP or ClickHouse Cloud — never laptop `localhost`, Docker service names, or private RFC1918 without SF private networking).
> - `DeployDatabase` does **not** provision a database. It only **validates and attaches the connection details for the user's existing database** to a deployment (and resolves the stored password server-side). If the user has no database yet, they must create one themselves first — do not imply StreamingFast will spin one up.
> - Never tell the user StreamingFast will "provision", "host", "spin up", or "create" a database for them. If they ask for a fully-managed database too, tell them they need to stand one up (e.g. ClickHouse Cloud, a managed Postgres, or self-hosted with public DNS) and then point the hosted sink at it.

## Supported output databases (hard limit)

**Hosted SQL sinks support only two engines:**

| Engine | Supported | Notes |
|---|---|---|
| **PostgreSQL** | ✅ | Database Changes **or** From proto definition |
| **ClickHouse** | ✅ | **From proto definition only** (no Database Changes) |

**Not supported on hosted sinks** (do not offer, deploy, or invent configs for them here):

- MySQL / MariaDB / SQLite / BigQuery / Snowflake / Redshift / DuckDB / any other SQL dialect
- `substreams-sink-kv` destinations
- `substreams-sink-files` / object storage / PubSub / webhooks as the hosted runner target
- **`GoogleCloudSqlPrivate`** — appears in the `OutputConfig` proto (`portal-api`) for private GCP Cloud SQL networking; **not** part of the standard agent-hosted Postgres/ClickHouse path. Do not invent Cloud SQL private configs unless product docs explicitly enable it for the user's org.

If the user wants KV, files, or a self-managed sink binary, stop and use `substreams-sink-deploy-local` or `substreams-sink` instead. If they need help choosing among SQL / KV / files, use the choice tree in `substreams-sql` (Step 1), then return here only when they commit to hosted **SQL** into Postgres or ClickHouse.

### Pre-flight: always choose the engine

#### How to ask (MANDATORY)

Same as other Substreams skills: **one question per turn**, **selectable choices**, always include **“Other / enter a custom answer”**. Do not batch engine + mode + deploy details into one message.

Before Deploy / DeployDatabase, if the engine is unknown, ask **only**:

> Which database engine should the hosted sink use?
> 1. **PostgreSQL**  
> 2. **ClickHouse**  
> 3. **Other / enter a custom answer**

If they pick Other with an unsupported engine, restate the hard limit (Postgres/ClickHouse only) and re-offer 1–2.

Then, **separate turn** for mapping mode when needed:

| Chosen engine | Modes (own turn) |
|---|---|
| **PostgreSQL** | **Database Changes** · **From proto definition** · **Other / custom** |
| **ClickHouse** | **From proto definition only** — do not offer Database Changes; explain briefly and proceed |

Never proceed with an engine outside that table.

**Order of questions for a new hosted deploy** (do not reorder):

1. **Output quality offer** (`OUTPUT_TEST_STATUS` — top of this skill) — **first**, always  
2. Engine (Postgres vs ClickHouse) if unknown  
3. Mapping mode if needed  
4. Then public spkg → Portal → DB fields → secret → Deploy  

If the user says “deploy to hosted ClickHouse” in one breath, still do **step 1 (output test offer) before** confirming deploy readiness.

## When to Use This Skill

- "Host my Substreams sink / run it for me / I don't want to manage a server."
- "Deploy this spkg to StreamingFast as a managed SQL sink."
- "Spin up a hosted Postgres/ClickHouse sink for my substreams."
- "Scale / pause / reconfigure / tear down my hosted deployment."
- "Is my hosted sink running? How far behind is it?" (status questions)

**Do NOT use this skill** when the user is:
- Running the sink on their own box → use `substreams-sink-deploy-local`
- Building the SQL module (`DatabaseChanges` or from-proto annotations) → use `substreams-sql`
- Writing the Substreams Rust → use `substreams-dev`
- Only asking about billing/usage/plan → use `portal-api`
- Asking for hosted **KV**, **files**, or a non-Postgres/ClickHouse database → explain the hard limit above and redirect

## How This Skill Relates to the Portal Skills

This skill owns the **sink-deployment workflow**; it does not re-document the API. Lean on:

- **`portal-api-jwt`** — authenticate. Hosted deployments are Portal API calls, so you need a Bearer access token. Run the device-code login; the token is pinned to one organization (the user picks it at approval) and carries their role.
- **`portal-api`** — the authoritative reference for every `HostedService` route, its request/response proto, the enum/state humanization, and the **Mutations Confirmation Protocol**. Read its "Hosted-deployment" sections for exact field shapes; this skill tells you *which* call to make and *how to fill it in* for a sink.

> All calls below are `POST {BASE_URL}/sf.portalapi.v1.HostedService/<Method>` with `Authorization: Bearer <access_token>` and an `organization_id` matching the token. `BASE_URL` defaults to `https://admin.streamingfast.io`. **Deploy / scale / reconfigure / undeploy / reset are mutations — confirm with the user first (per the `portal-api` Confirmation Protocol); deploying the hosted sink runner creates billable infrastructure. `DeployDatabase` does not provision a database — it only attaches the user's existing database connection.**

## Prerequisites

0. **`OUTPUT_TEST_STATUS` is set** (`tested_ok` | `user_verified` | `skipped`) via the **⛔ STOP** gate at the top of this skill — not inferred from “we built the package.”
1. **A built `.spkg` that is publicly reachable** (hard gate **before** `Deploy` / creating the hosted sink):
   - **Preferred for hosted Deploy:** public HTTPS URL the runner can `GET` without auth.
     - After `substreams publish`, use the registry download URL (verified working for sink runners):
       `https://api.substreams.dev/v1/packages/<package-name>/<version>`
       Example: `https://api.substreams.dev/v1/packages/raydium-clmm/v0.1.4`
     - Or a **GitHub Release** asset / public object storage URL.
   - **`substreams_dev_id` (e.g. `raydium-clmm@v0.1.4`)** — optional shorthand. **Do not rely on it as the only source for hosted sinks:** some runner versions mis-parse `substreams-dev://name@version` as a package name and fail with a registry regexp error. Prefer `spkg.url` with the API download URL above.
   - **Verify before Deploy:** `curl -sSIL` / download the URL; body size should match the local `.spkg` (binary), not an HTML page.
   - **Not allowed:** local filesystem paths, private GitHub/repo URLs, localhost, or anything requiring the user's machine. If the package is only local, **stop Deploy**, help publish/release, then continue.
   - **PostgreSQL + Database Changes:** module emits `proto:sf.substreams.sink.database.v1.DatabaseChanges` (build with `substreams-sql`).
   - **PostgreSQL or ClickHouse + From proto definition:** custom proto (not `DatabaseChanges`); ClickHouse never uses Database Changes. Follow `substreams-sql` for PK/ORDER BY and **ClickHouse reserved column names**.
2. **No API key to supply** — data-plane key is auto-created per deployment (`Deployment.api_key_id`); separate from Portal Bearer.
3. **A logged-in session** — Portal access token + pinned `organization_id` (`portal-api-jwt`). Login: **wait for user confirmation** — no background poll (see `portal-api-jwt`).
4. **A PostgreSQL or ClickHouse database the user already runs** — StreamingFast never hosts or provisions it. Collect the connection fields **one by one**; `DeployDatabase` only attaches/validates that user-provided connection (it is **not** a provisioning step). Host must be **reachable from StreamingFast’s network** (not laptop `localhost` / Docker-only DNS). If the user has no database yet, they must create one (ClickHouse Cloud, managed Postgres, or self-hosted with public DNS) **before** deploying — do not imply StreamingFast will create it.

## Pick the Deployment Type

`deployment_request` is a oneof — choose one:

| Type | Use for | Output module proto |
|---|---|---|
| `sink_sql_deployment` | Hosted SQL into **PostgreSQL or ClickHouse only** (the common case) | `DatabaseChanges` → standard mode (**Postgres only**); any other proto → From proto definition (Postgres or ClickHouse) |
| `foundational_store_deployment` | A foundational store keyed by a proto type (advanced; e.g. Solana SPL account owners) — **not** a general-purpose SQL/KV/files sink | the `type_url` you specify |

`module_output_type` decides SQL mode: exactly `proto:sf.substreams.sink.database.v1.DatabaseChanges` runs standard Database Changes (**valid only when `outputConfig` is Postgres**); anything else runs From proto definition. For ClickHouse, always use a non-`DatabaseChanges` module output type.

## Workflow

### 0. Output quality gate (`OUTPUT_TEST_STATUS`)

Execute the **⛔ STOP — Output quality gate** at the top of this skill **before any later step**. If the flag is unset, **only** run that offer or give the **`substreams run` command** (no sink). Do not start Portal auth, mint `deployment_id`, collect DB secrets, or ask “ready to deploy?” until the flag is set.

Typical sequence: **offer → paste `substreams run` command → user reviews stdout → OK → then step 1.**

### 1. Public spkg (before any Deploy)

Publish if needed (`substreams publish`), then set Deploy to a **public HTTPS** package URL (prefer `https://api.substreams.dev/v1/packages/<name>/<version>`). Do not call `Deploy` with a local path or an unverified `substreams_dev_id` alone.

### 2. Authenticate

Run `portal-api-jwt` for access token + `organization_id`. Reuse a valid token; refresh rather than re-prompting. Mutations need **OWNER/ADMIN**.

When login is needed: show the login callout **alone**, **end the turn**, and **wait for the user to say they approved**. **Do not** background-poll Portal (or The Graph Market / other browser hand-offs). See `portal-api-jwt` Step 3.

### 3. Collect database connection (one field per turn)

**Never ask for a DSN** — collect discrete fields (below), not a connection string.

If the user brings their own DB, ask **one question at a time** (choices + **Other / custom**):

host → port → user → database → schema (Postgres) → sslmode/secure. **Never ask for the password in chat.**

**Host reachability gate (before Deploy):**

| Prefer | Notes |
|---|---|
| Public hostname / IP | Routable from the internet |
| ClickHouse Cloud | Host + port **9440** + `secure: true` (native TLS) |
| Self-hosted with public DNS | Native TCP 9000 (or TLS port if configured) |

The database is always the user's own — StreamingFast does not provision one. Whether you attach it via `DeployDatabase` or inline in `Deploy`, still stage the password via step 4 first.

### 4. Stage the database password (never inline; no background poll)

Password must never ride in Deploy. User stages it in the browser:

1. **Mint `deployment_id`** via `CreateDeployment` (server-generated — **never invent** a slug).
2. Send them to the **Portal UI** secret page — **not** `admin.streamingfast.io` (API host only; `/sinks/...` returns **404** there).

   **Canonical secret URLs** (use these full strings):

   | Engine | URL |
   |---|---|
   | PostgreSQL | `https://thegraph.market/sinks/<deployment_id>/secret?output=postgres` |
   | ClickHouse | `https://thegraph.market/sinks/<deployment_id>/secret?output=clickhouse` |

   Alternates that also work: `https://app.streamingfast.io/sinks/<deployment_id>/secret?output=postgres|clickhouse`.

   **Do not** use `https://admin.streamingfast.io/sinks/...`.

   Emit this callout and **stop the turn** (no parallel poll):

   > ### 🔑 Enter your {DB} password securely
   >
   > Open this page and enter the password for {DB} user `{user}`:
   >
   > {secret_url}
   >
   > It's stored server-side under `{key}` — it never passes through this chat.
   >
   > ### 👉 Tell me once you've saved it — only then will I verify and continue.

3. **After the user confirms**, call `HasDeploymentSecret` **once** (`postgres_password` or `clickhouse_password`). If `exists: false`, ask them to save again / re-confirm — **no** sleep/poll loop.

Optional `DeployDatabase` (after secret exists) — this **attaches and validates the user's existing database connection**, it does not create a database. `use_stored_secret: true` and empty password. Note the request only carries a `postgres_spec` field (there is no `clickhouse_spec`); for ClickHouse, supply the connection inline in `Deploy` step 5 instead:

```bash
# $DEPLOYMENT_ID = value returned by CreateDeployment (server UUID — do not invent)
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.HostedService/DeployDatabase" \
  -H "Authorization: Bearer $ACCESS_TOKEN" -H "Content-Type: application/json" \
  -d '{
        "deployment_id": "'"$DEPLOYMENT_ID"'",
        "organization_id": "'"$ORG_ID"'",
        "use_stored_secret": true,
        "postgres_spec": { "server": "...", "port": 5432, "user": "...", "database": "...", "schema": "public", "sslmode": "require" }
      }'
```

### 5. Deploy

**Abort checklist (all must pass before you even ask for deploy confirmation):**

| Check | If missing |
|---|---|
| `OUTPUT_TEST_STATUS` is `tested_ok`, `user_verified`, or `skipped` | Go to workflow step 0 — do **not** ask “ready to deploy?” |
| Public spkg `url` (preferred) verified | Publish / fix URL first |
| `deployment_id`, name, network, output_module, module_output_type | Collect first |
| Non-secret DB fields + `use_stored_secret` path | Steps 3–4 |

Only then: require public spkg **`url`** (preferred) or verified `substreams_dev_id`, `deployment_id`, `name`, `network`, `output_module`, `module_output_type`, non-secret DB fields, `replica` (default `1`). `use_stored_secret: true`, empty `password`. No API key.

When echoing the plan for confirmation, **include the quality line**, e.g.:

> Output check: **user ran `substreams run` (stdout)** / **prior `substreams run` verified** / **skipped** (`OUTPUT_TEST_STATUS=…`) — **not** a local DB sink. Deploy hosted sink `name` → ClickHouse/Postgres at `host` …

If you cannot fill that line from a real offer turn this session, **stop** and run step 0.

**Network string (sink_sql):** use the Substreams network id, e.g. `solana` / `solana-mainnet-beta` for Solana, `mainnet` / `ethereum-mainnet` for Ethereum — match what `substreams run --network` expects for that package.

**PostgreSQL** (Database Changes example — use From-proto `module_output_type` when not emitting `DatabaseChanges`):

```bash
# Prefer spkg.url (API download) for hosted runners.
# $DEPLOYMENT_ID from CreateDeployment — never invent a slug.
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.HostedService/Deploy" \
  -H "Authorization: Bearer $ACCESS_TOKEN" -H "Content-Type: application/json" \
  -d '{
        "deployment_id": "'"$DEPLOYMENT_ID"'",
        "name": "My ETH transfers sink",
        "organization_id": "'"$ORG_ID"'",
        "use_stored_secret": true,
        "deployment_request": {
          "sink_sql_deployment": {
            "spkg": { "url": "https://api.substreams.dev/v1/packages/my-package/v0.1.0" },
            "network": "ethereum-mainnet",
            "replica": 1,
            "execution_config": {
              "start_block": 12000000,
              "output_module": "db_out",
              "module_output_type": "proto:sf.substreams.sink.database.v1.DatabaseChanges"
            },
            "outputConfig": { "postgres": { "server": "...", "port": 5432, "user": "...", "database": "...", "schema": "public", "sslmode": "require" } }
          }
        }
      }'
```

**ClickHouse** (From proto definition only — never `DatabaseChanges`; connection is always inline in Deploy, not via `DeployDatabase`):

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.HostedService/Deploy" \
  -H "Authorization: Bearer $ACCESS_TOKEN" -H "Content-Type: application/json" \
  -d '{
        "deployment_id": "'"$DEPLOYMENT_ID"'",
        "name": "My Solana CH sink",
        "organization_id": "'"$ORG_ID"'",
        "use_stored_secret": true,
        "deployment_request": {
          "sink_sql_deployment": {
            "spkg": { "url": "https://api.substreams.dev/v1/packages/my-package/v0.1.0" },
            "network": "solana",
            "replica": 1,
            "execution_config": {
              "start_block": 250000000,
              "output_module": "db_out",
              "module_output_type": "proto:my.package.v1.Events"
            },
            "outputConfig": {
              "clickhouse": {
                "server": "xxx.clickhouse.cloud",
                "port": 9440,
                "user": "default",
                "database": "default",
                "secure": true
              }
            }
          }
        }
      }'
```

Notes:
- **No `password` in `outputConfig`** — it's staged in vault (step 4) and resolved server-side via `use_stored_secret: true`. Sending an inline password from an agent token is rejected.
- `outputConfig` is intentionally **camelCase** in the JSON (it's named that in the proto); many other response fields may arrive as **camelCase** from Connect JSON — accept both snake_case and camelCase when parsing (`deploymentId` / `deployment_id`, etc.).
- **`Deploy` success** is often HTTP **200** with an **empty body** `{}` (empty `DeploymentResult`). Treat that as success; confirm with `GetDeploymentState`, not by expecting a rich Deploy payload.
- **Output DB cold start** — the server runs a connection probe against the output DB before it accepts the deploy. A serverless ClickHouse (and some Postgres) idles its compute and can take **1–2 minutes to wake**, so the first probe may come back as a connection **timeout / refused / unreachable** error. This is transient: **retry the `Deploy` call** (2–3 attempts, waiting ~30–60 s between them) rather than reporting failure. Tell the user the DB is waking up and you're retrying. Only surface a hard failure after the retries are exhausted, or immediately if the error is clearly not transient (auth rejected, host not found, bad credentials).
- For a foundational store, swap `sink_sql_deployment` for `foundational_store_deployment` with its `config` (`type_url`, `network`, optional `resources`, `token`, `substreams_parameters`).

### 6. Watch it roll out

Poll `GetDeploymentState` until it's healthy; lead with the headline status and per-pod execution detail (state, `current_block`/`head_block`, `head_block_time_drift`). Use `portal-api`'s state humanization (Deploying / Running / Error; Live / Catching up / Failing).

```bash
curl -sS -X POST "$BASE_URL/sf.portalapi.v1.HostedService/GetDeploymentState" \
  -H "Authorization: Bearer $ACCESS_TOKEN" -H "Content-Type: application/json" \
  -d '{"deployment_id":"'"$DEPLOYMENT_ID"'","organization_id":"'"$ORG_ID"'"}'
```

If it goes to `ERROR` or crashloops, pull `GetDeploymentEvents` (recent lifecycle events) and `Logs` (`tail_lines`, `previous:true` for a crashed container) to diagnose. List everything with `ListDeployments`. **If the logs/events show an output-DB connection timeout** (a serverless ClickHouse/Postgres still waking up), treat it as transient: keep polling for another minute or two — the sink retries the connection itself once the DB is up — before treating it as a real failure.

### 7. Operate

- **Scale / pause** — `SetReplica` with `count` (0 pauses processing).
- **Reconfigure** — `UpdateDeploymentConfig` (new spkg, execution config, output, or `api_key_id`; leave a field empty to keep it). `restart_from_scratch:true` resets the cursor (foundational-store only).
- **Reset (destructive)** — `ResetDeployment`; `drop_schema:true` drops the schema, `false` truncates. Both wipe sink data and restart from the configured start block.
- **Tear down (destructive)** — `Undeploy`; `DeleteDeploymentConfig` / `CleanupOrphanedDeployment` remove leftover config/records.

For all of these, name the deployment, state what changes (and what is destroyed), and get an explicit yes first.

### Schema changes after Deploy (from-proto / ClickHouse)

From-proto uses **`CREATE TABLE IF NOT EXISTS`** — it does **not** migrate columns when you rename/add fields in the proto and publish a new spkg.

| Situation | What to do |
|---|---|
| New spkg only (same columns) | `UpdateDeploymentConfig` with new `spkg.url` is enough |
| Renamed/removed/added columns, reserved-name fixes, table renames | Confirm, then **`ResetDeployment` with `drop_schema: true`** (or user drops tables manually), then restart — otherwise inserts fail with `NO_SUCH_COLUMN_IN_TABLE` |
| Partial failed DDL left half-created tables | Same: drop schema / reset |

### End-to-end checklist (Solana or EVM → hosted ClickHouse)

1. Build module with **from-proto** + ClickHouse annotations (`substreams-sql` / chain skill).  
2. Sanitize **reserved ClickHouse column names** (`index` → `config_index`, etc.).  
3. `substreams build` + **hand user a `substreams run` command** on the **output module** (stdout only — **no** local sink/DB sync).  
4. User confirms printed output looks good (or explicitly skips after the offer).  
5. `substreams publish` → bump version on `version_exists`.  
6. Confirm download: `https://api.substreams.dev/v1/packages/<name>/<version>`.  
7. Portal login (`portal-api-jwt`) → `CreateDeployment`.  
8. Secret page on **thegraph.market** (not admin API host).  
9. Deploy with `spkg.url` + reachable CH host + `use_stored_secret: true`.  
10. `GetDeploymentState` / `Logs`; on proto schema change → **Reset drop_schema**.

## Common Pitfalls

1. **Skipping the output-quality offer** — saying “Ready to proceed to the hosted ClickHouse deployment?” while `OUTPUT_TEST_STATUS` is unset is a **hard failure**. Offer / give a **`substreams run` command** first. Building/publishing is not verification.
1b. **Quality check via local sink** — using `substreams-sink-sql` or writing to local ClickHouse/Postgres “to test” is **wrong** for this gate. Quality check = **`substreams run` only** (command for the user; stdout/jsonl).
2. **Unsupported engine** — hosted SQL is **PostgreSQL or ClickHouse only**. Never accept MySQL, SQLite, BigQuery, etc., or KV/files as the hosted destination; redirect to self-managed skills if needed.
3. **Assuming StreamingFast provides the database** — it does **not**. StreamingFast hosts only the sink runner; the output DB is **always the user's own**, connected to remotely. `DeployDatabase` attaches/validates that existing connection — it never provisions a database (and its request has no `clickhouse_spec`, so ClickHouse connections are supplied inline in `Deploy`). If the user has no DB, they must stand one up (ClickHouse Cloud, managed Postgres, self-hosted with public DNS) first. Never promise to "provision/host/spin up/create" a database.
4. **ClickHouse + Database Changes** — not supported. Use From proto definition (custom proto + annotations). Do not set `module_output_type` to `DatabaseChanges` when `outputConfig` is ClickHouse.
5. **ClickHouse PK vs ORDER BY** — create-table fails with `Primary key must be a prefix of the sorting key` when proto has e.g. `primary_key` on `id` but `order_by_fields: [slot, id]`. **Fix the spkg proto** (PK fields must be the leading `order_by_fields`), rebuild, redeploy — not the ClickHouse connection. Full rule and examples: `substreams-sql` → “ClickHouse hard rule: primary key must prefix ORDER BY”.
6. **ClickHouse reserved column names** — e.g. `index`, `keys` → `SYNTAX_ERROR` on CREATE. Rename in proto before first deploy (`substreams-sql`).
7. **`substreams_dev_id` parse failure** — logs show `invalid Substreams Registry short package identifier "substreams-dev://..."`: switch Deploy/Update to `spkg.url` = `https://api.substreams.dev/v1/packages/<name>/<version>`.
8. **Secret page 404 on admin.streamingfast.io** — use `https://thegraph.market/sinks/<id>/secret?output=clickhouse` (or `app.streamingfast.io`).
9. **Unreachable host** — `localhost` / Docker service names fail from hosted runners.
10. **Schema drift after proto rename** — `NO_SUCH_COLUMN_IN_TABLE` → Reset with `drop_schema: true` (confirm first).
11. **Wrong output proto** — a SQL sink whose module doesn't emit `DatabaseChanges` runs in From proto definition mode (tables derived from your proto). If the user expected `db_out`/`DatabaseChanges` on **Postgres**, send them to `substreams-sql` first.
12. **`organization_id` mismatch** — the Bearer token is pinned to one org; every call's `organization_id` must equal it, or it's rejected. Re-login to target a different org.
13. **`permission_denied` on a mutation** — the logged-in user isn't OWNER/ADMIN of the org. Reads still work.
14. **`unauthenticated` mid-session** — the access token expired; refresh via `portal-api-jwt` and retry (don't re-prompt).
15. **spkg not public** — local path or private URL fails. Publish and use API/public HTTPS URL **before** Deploy.
16. **Background poll during login/password** — forbidden. Wait for user confirmation, then one check (`DeviceToken` / `HasDeploymentSecret`).
17. **Batched DB questions** — ask host, port, user, database, schema, SSL **one at a time**.
18. **`api_key_id` vs Portal token** — auto-created sink data-plane key ≠ Portal Bearer; don't conflate.
19. **Output DB cold start** — serverless DB may take 1–2 min; retry `Deploy` a few times; distinguish from real auth/host errors.

## Resources

- `portal-api` — full `HostedService` proto, state enums, confirmation protocol.
- `portal-api-jwt` — device-code login, token refresh.
- `substreams-sql` — build the SQL module (Database Changes on Postgres; From proto definition on Postgres/ClickHouse). Includes the sink-type choice tree (SQL vs KV vs files).
- `substreams-dev` / `substreams-testing` — data-plane auth and `substreams run` / quality verification before Deploy.
- `substreams-sink-deploy-local` — run the same sink yourself instead of hosting it.
- https://docs.substreams.dev/how-to-guides/sinks
