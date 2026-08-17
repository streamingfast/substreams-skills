---
name: substreams-dev
description: Cross-cutting Substreams development — manifests, module graphs (map/store/index), protobuf, performance, debugging. For Ethereum/EVM contract decoding use substreams-ethereum; for Solana programs use substreams-solana; for SQL sinks use substreams-sql.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, opencode, vscode, windsurf]
metadata:
  version: 1.6.0
  author: StreamingFast
  documentation: https://substreams.streamingfast.io
---

# Substreams Development Expert

Expert assistant for **cross-cutting** Substreams development: manifests, module graphs, protobuf, build/run, and performance. Chain-specific contract/program work lives in dedicated skills.

## Skill routing (load the right skill)

| User goal | Skill |
|---|---|
| **Ethereum / EVM contracts** (ABI, logs, eth_call, Uniswap, ERC-20) | **`substreams-ethereum`** |
| **Solana programs** (walk_instructions, SPL, Anchor, Raydium, …) | **`substreams-solana`** |
| SQL sink module / Postgres / ClickHouse | `substreams-sql` |
| Custom app sink (Go/JS/…) | `substreams-sink` |
| Run sink yourself / host on StreamingFast | `substreams-sink-deploy-local` / `substreams-hosted-sink` |
| Porting a subgraph or contract to Substreams | `substreams-convert` |
| Testing strategy | `substreams-testing` |
| Manifest, stores, indexes, cloning, graph-out basics | **this skill** (`substreams-dev`) |

**Always establish the target chain early.** If the user has not said Ethereum vs Solana (or another family), ask before writing code — then follow the chain skill for implementation details.

## Core Concepts

Substreams indexes blockchain data through composable Rust modules compiled to WASM, executed in parallel by the engine, with cursor-based reorg handling.

1. **Manifest** (`substreams.yaml`): modules, networks, dependencies
2. **Modules**: map (transform), store (aggregate), index (filter)
3. **Protobuf**: typed schemas for inputs and outputs
4. **WASM**: Rust compiled to `wasm32-unknown-unknown`

## Project Structure

Generic layout — **both EVM and Solana use `build.rs`** for domain protobufs via `prost_build`. EVM additionally uses `abi/` + Abigen in the same `build.rs`; Solana does not.

```
my-substreams/
├── substreams.yaml              # Manifest (manual) — must include protobuf: + binaries:
├── README.md                    # Package docs for substreams.dev registry (manual)
├── Cargo.toml                   # Rust dependencies (manual)
├── build.rs                     # prost_build (required); EVM also runs Abigen here
├── proto/events.proto           # Schema definitions (manual)
├── src/
│   ├── lib.rs                   # Rust module code + mod pb { include!(OUT_DIR) } (manual)
│   └── pb/                      # Optional: only if using substreams protogen (auto)
└── target/                      # Build output (gitignored)
```

**Do not hand-write generated proto Rust.** Prefer `build.rs` + `include!(concat!(env!("OUT_DIR"), "/…"))` (Solana T5.x / most EVM examples). If you use `substreams protogen`, the `src/pb/` tree is auto-generated — never create it by hand.

```bash
substreams protogen  # Proto bindings only (fast iteration)
substreams build     # Full WASM + spkg
```

Chain-specific scaffolding (full Solana / EVM Cargo + `build.rs` + packable manifest): **`substreams-solana`** / **`substreams-ethereum`**.

## Prerequisites

- **substreams**: core CLI for building, running, deploying
- **buf**: required by `substreams build` for protobuf codegen
- **Rust**: `rustup target add wasm32-unknown-unknown`

### Authentication

`substreams run` against hosted endpoints requires auth. Get an API key from [The Graph Market](https://thegraph.market).

```bash
substreams auth   # interactive; exchanges the key and stores the token locally
```

Alternative: `export SUBSTREAMS_API_KEY="…"` (or `SUBSTREAMS_API_TOKEN="<jwt>"` for a bearer token directly).

If you send the user to a browser login, treat it like Portal device login: show the link, **end the turn**, and **wait for the user to confirm** — do **not** background-poll or busy-wait.

> **This is data-plane auth, not Portal auth.** `SUBSTREAMS_API_KEY` / `SUBSTREAMS_API_TOKEN` authenticate `substreams run` against the **streaming data endpoints**. The StreamingFast **Portal** admin API (billing, usage, hosted deployments) is separate — see `thegraph-market-api`. A Portal bearer token will **not** authenticate `substreams run`. Reach for that skill only for plan, quota, usage, or deployment status.

## Pre-flight: Clarifying Under-Specified Requests

Before writing code, confirm the items below. If any is missing or ambiguous, **ask** — do not assume silently.

### How to ask (MANDATORY for all Substreams skills)

1. **One question per turn** — never batch a questionnaire. Ask, wait, then ask the next.
2. **Offer explicit choices** — selectable options (platform choice UI when available; else a numbered list).
3. **Always include a custom-answer choice** — the last option must allow free text ("Other / enter a custom answer").
4. **Skip what is already known** — do not re-ask what the user stated.
5. **Order matters** — walk the list top-to-bottom.

**Bad:** one message with 6 free-text questions and no options.
**Good:** "Which chain?" → options → answer → next turn "Where should output go?" → options including custom.

| Required input | Why it matters | Example choices (always end with custom) |
|---|---|---|
| **Target chain** | Routes to `substreams-ethereum` vs `substreams-solana` | Ethereum mainnet · Other EVM · Solana mainnet · **Other / custom** |
| **Contract / program ID(s)** | EVM address vs Solana program ID | Known protocol names · **Other / paste address** |
| **Data to capture** | Events / instructions / state | Protocol presets · **Other / describe fields** |
| **Output / sink destination** | See destination list below | Full list · **Other / custom** |
| **Block or slot range** | `initialBlock` and test range | Suggested windows · Head / recent · **Other / enter range** |
| **Thresholds or filters** | Min value, mint allowlist, etc. | None · Suggested filter · **Other / custom** |
| **Block sparsity** | If the target appears in only *some* blocks, a **block index filter** is a near-mandatory cost optimization | Sparse (index filter) · Every block · **Other / not sure** |

### Output destination — ask as its own turn (ALWAYS offer all of these)

| Choice | What it means | Follow-on skill |
|---|---|---|
| **`substreams run` only** | Typed protobuf; verify with CLI/GUI first | this skill + chain skill |
| **SQL — self-managed** | User runs `substreams sink postgres`/`clickhouse` into Postgres/ClickHouse | `substreams-sql` → `substreams-sink-deploy-local` |
| **SQL — StreamingFast hosted** | StreamingFast runs the SQL sink (Portal; Postgres or ClickHouse only) | `substreams-sql` → **`substreams-hosted-sink`** |
| **The Graph / graph-out** | `EntityChanges` for subgraph consumption | `substreams-sink` |
| **Custom app sink** | Go/JS/Python/Rust consumer | `substreams-sink` |
| **Not sure yet** | Default to run-only; add a sink later | this skill |
| **Other / enter a custom answer** | Free-text destination | route from answer |

**SQL is two destinations, not one:** if the user says only "SQL", ask **in a separate turn** self-managed vs StreamingFast hosted (plus custom). Hosted still needs a correct SQL module (`substreams-sql`).

Once the chain is known, **load and follow** `substreams-ethereum` or `substreams-solana`. Keep this skill for module graph design, manifest rules, and performance. **Chain and sink skills use the same one-question + choices + custom protocol.**

**If the prompt is concrete and complete**, skip the checklist and build immediately.

## Discovering Existing Packages (Registry Search API)

**Before building from scratch, search the registry — an existing package may already index the data you need.** Public, no auth required.

**Endpoint:** `GET https://substreams.dev/v1/registry/packages`

| Param | Description |
|---|---|
| `query` | Free-text search over package name/keywords |
| `network` | Filter by network (e.g. `mainnet`) — **see the caveat below** |
| `organization` | Filter by organization slug |
| `user` | Filter by publisher user login |
| `featured` | `true` to restrict to featured packages |
| `page` | 1-based page number (default `1`) |
| `page_size` | Results per page (default `24`, max `100`; `pageSize` also works) |

```bash
curl "https://substreams.dev/v1/registry/packages?query=uniswap&page_size=5"
```

**Response shape — fields are omitted, not empty.** This is proto3 JSON: default-valued and absent fields do not appear at all.

```json
{
  "packages": [
    {
      "name": "uniswap_v3",
      "slug": "uniswap-v3",
      "organization": { "name": "Uniswap", "slug": "uniswap" },
      "repository": "https://github.com/...",
      "downloads": 7,
      "releaseCount": 1,
      "latestVersion": "v0.1.0",
      "network": "mainnet",
      "reference": "uniswap-v3@v0.1.0",
      "spkg": "https://spkg.io/v1/packages/uniswap-v3/v0.1.0"
    }
  ],
  "hasMore": true
}
```

**Parse defensively — every one of these is real:**
- **`hasMore` is absent when there are no more pages** (not `false`). Treat missing as "done".
- **Zero results returns `{}`** — not `{"packages": []}`. Treat missing `packages` as an empty list.
- **`network`, `organization`, and `repository` are omitted when absent.** `ethereum-common` has no `network` field.
- **`name` and `slug` differ**: `name` may use underscores (`uniswap_v3`), `slug` uses hyphens (`uniswap-v3`). **URLs are built from the `slug`.**

> **Do not add `network=mainnet` reflexively.** It excludes packages that have no `network` field — including `ethereum-common`, the foundational package this skill tells you to import. `?query=ethereum-common` returns it; `?query=ethereum-common&network=mainnet` returns `{}`.

**`spkg` vs `reference` — pick the right one:**
- **`spkg`** — full package URL (`https://spkg.io/v1/packages/<slug>/<version>`). **Use this for execution and for `imports:`**: `substreams run <spkg> <module>`, `substreams gui <spkg>`, `substreams info <spkg>`, `imports: { eth_common: <spkg> }`.
- **`reference`** — short `<slug>@<version>` notation. **Display only.** The CLI rewrites `name@version` to `https://substreams.dev/v1/packages/<name>/<version>`, which returns an HTML 404. Working download hosts are **`spkg.io`** and **`api.substreams.dev`** only. There is no `@latest` tag — pin the version from `latestVersion` / `spkg`.

Both are absent when no release version is known.

**Rate limits:** the API is rate-limited per IP. Over-limit requests return `429` with a `Retry-After` header — honor it and back off. The full contract is published at `GET https://substreams.dev/v1/registry/openapi.yaml` (source of truth for fields; it does not document the rate limits).

## Creating a New Project

> **Migrating an existing project?** Load **`substreams-convert`** if porting a subgraph or Solana/EVM contract rather than starting fresh.

1. **Initialize**: `substreams init`, or write the manifest manually
2. **Define schema**: `.proto` files for your data structures
3. **Implement modules**: Rust handlers in `src/lib.rs`
4. **Build**: `substreams build` → `.spkg`
5. **Test**: `substreams run` over a small range (~1000 blocks)
6. **Document**: `README.md` for the registry (below)
7. **Publish**: to the registry, or deploy as a service

### README for substreams.dev Registry

Every published package **must** ship a `README.md` — it is the package's front page on [substreams.dev](https://substreams.dev), and its presence silences the `README (package.doc) not found` warning.

Required sections: **title** (matching `package.name`), **one-line description**, **Overview** (2-3 sentences: what data, what chain, intended use), **Modules table** (every `name:` from the manifest, with kind + output type + description — consumers need this to know what to `substreams run`), **Prerequisites**, and a **Quick Start** with a real `substreams run` command over a real block range (not a placeholder).

Do NOT add "Contributing" or "License" sections — the registry pulls license from the manifest.

### Publishing

```bash
substreams registry verify ./substreams.yaml   # optional preflight
substreams build
substreams publish ./substreams.yaml --yes     # or path to .spkg
```

| Issue | Action |
|---|---|
| `version_exists` / "Release version already exists" | Bump **`package.version`** patch (`v0.1.0` → `v0.1.1`), rebuild, publish |
| Package name collision / ownership | Choose a unique `package.name`, or publish under a team (`--team-slug`) |
| After successful publish | Web URL is `https://substreams.dev/packages/<name>/<version>` |

**Hosted SQL sink package source (important):** after publish, for StreamingFast hosted Deploy prefer:

```text
https://api.substreams.dev/v1/packages/<package-name>/<version>
```

Verify the URL returns a binary `.spkg` (size ≈ local file), not HTML. Prefer this as `spkg.url` over `substreams_dev_id` alone — some hosted runners mis-parse `substreams-dev://name@version`. Full flow: `substreams-hosted-sink` + `thegraph-market-api`.

## Module Types

Full details: [references/module-types.md](./references/module-types.md).

**Map** — transforms input to output:
```yaml
- name: map_events
  kind: map
  inputs:
    - source: sf.ethereum.type.v2.Block
  output:
    type: proto:my.types.Events
```

**Store** — aggregates across blocks:
```yaml
- name: store_totals
  kind: store
  updatePolicy: add
  valueType: int64
  inputs:
    - map: map_events
```

> **Store inputs use block form, never inline commas.** `- store: x, mode: deltas` is a **YAML parse error** — a plain scalar may not contain `": "`. Write:
> ```yaml
> inputs:
>   - store: store_totals
>     mode: deltas      # or: get
> ```

**Index** (`kind: blockIndex`) — emits per-block `Keys` so downstream modules can **skip blocks**. The handler is `#[substreams::handlers::map]` returning `Keys`. See "Block & Transaction Filtering" below.

### `initialBlock`

```yaml
modules:
  - name: map_events
    kind: map
    initialBlock: 18000000   # ✅ start of your test/data range
    # NOT: 12369621          # ❌ protocol genesis — forces full backfill on every run
```

Pin `initialBlock` to the first block your downstream consumer actually needs. The runtime starts at `max(--start-block, initialBlock)` and walks forward; stores must catch up from `initialBlock` on each cold start, so a deep genesis pin turns a 100-block test into a multi-hour backfill.

## Block & Transaction Filtering (Cost-Critical)

**Whenever a Substreams targets only a subset of blocks — a specific contract, program, event signature, account, or transaction type — add a block index filter. Be aggressive about this.** It is the biggest win available for both your iteration speed and your bill: you are billed for blocks the engine actually processes, so a contract active in 0.5% of blocks costs roughly 0.5% as much, and backfills that would take hours finish in minutes.

**The pattern (three pieces):**

```yaml
# 1. Index module — kind `blockIndex`, output `Keys`
- name: index_events
  kind: blockIndex
  inputs:
    - map: all_events
  output:
    type: proto:sf.substreams.index.v1.Keys

# 2. Consuming module — declares blockFilter to skip non-matching blocks
- name: filtered_events
  kind: map
  blockFilter:
    module: index_events
    query:
      params: true          # SQE query from `params` (or: string: "<expr>")
  inputs:
    - params: string
    - map: all_events
  output:
    type: proto:my.types.Events
```

The index handler is a **`map` handler** returning `Keys` (a `repeated string` of labels you choose):

```rust
#[substreams::handlers::map]
fn index_events(events: Events) -> Result<Keys, Error> {
    let mut keys = Keys::default();
    for e in events.events {
        if let Some(log) = e.log {
            if let Some(t0) = log.topics.get(0) {
                keys.keys.push(format!("evt_sig:0x{}", Hex::encode(t0)));
            }
            keys.keys.push(format!("evt_addr:0x{}", Hex::encode(&log.address)));
        }
    }
    Ok(keys)
}
```

**Query (SQE):** boolean expression over keys — `&&`, `||`, `-` (not), `( )` grouping. A bare term must match a key exactly: `"evt_addr:0xA && -evt_addr:0xspam"`.

**Critical rules:**
- The index does **nothing automatically**. Only a module with an explicit `blockFilter` gets blocks skipped — listing the index as a dependency is not enough.
- Query namespace must match the emitted key prefix exactly (`evt_addr:` vs `address:`), and values match by **literal equality** — use 0x-prefixed **lowercase** hex (EVM checksum/mixed-case addresses never match).
- **Don't reinvent.** Most chains ship a foundational package whose `filtered_*` modules already apply the `blockFilter` *and* return only matching records — depend on those via **spkg URL** (short `name@version` imports 404):
  ```yaml
  imports:
    eth_common: https://spkg.io/v1/packages/ethereum-common/v0.3.3
  # then: map: eth_common:filtered_events
  ```
  You **must override the default params** query, or you silently emit the default's data. In-handler filtering is only needed when you roll your own `blockFilter`, or for Solana instruction-level filtering (transactions are pre-filtered, instructions within them are not).

**Full guide** (SQE syntax, `params` vs `string`, `use` inheritance, foundational indexes, decision flowchart): [references/block-filtering.md](./references/block-filtering.md).

## Manifest Reference

Full spec: [references/manifest-spec.md](./references/manifest-spec.md).

```yaml
specVersion: v0.1.0
package:
  name: my-substreams
  version: 1.6.0
  url: https://github.com/myorg/my-substreams
  description: What this substreams does

protobuf:
  files:
    - events.proto
  importPaths:
    - ./proto

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/my_substreams.wasm

network: mainnet   # see references/networks.md
```

> **`version` requires a `v` prefix.** Use `v0.1.0`, not `0.1.0`. The error (`version "0.1.0" should match Semver`) is misleading — both are valid semver, but Substreams mandates the `v` form. Applies to `package.version` only.

> **Always set `url` + `description`, add a `README.md`, and never use `package.doc`.** Omitting these produces a wall of non-fatal warnings on every build — confusing, looks broken, isn't. `package.doc` is **deprecated**; delete it if a scaffold includes it and write a `README.md` instead (the packager picks it up automatically). Module-level `doc:` fields are fine.

**WASM bindgen shims** — some crates (`solana_program`, `alloy`, `chrono`) emit bindgen imports under `wasm32-unknown-unknown`. To compile:

```yaml
binaries:
  default:
    type: wasm/rust-v1+wasm-bindgen-shims
```

The shims allow compilation but do not implement the underlying functionality — avoid calling those imports at runtime.

## Rust Module Development

### Cargo.toml (cross-cutting)

Always set `crate-type = ["cdylib"]`, release LTO, and matching `prost` / `prost-types` majors.

| Chain | Skill for full Cargo.toml | Core crates |
|---|---|---|
| **EVM** | `substreams-ethereum` | `substreams = "0.7"`, `substreams-ethereum = "0.11"`, `ethabi = "18"`, `hex` |
| **Solana** | `substreams-solana` | `substreams = "0.7"` + `substreams-solana = "0.15"` (legacy: `0.6` + `0.14.x` only — never mix majors), `bs58`, `sha2` |
| **SQL DatabaseChanges** | `substreams-sql` | `substreams-database-change = "4"` (requires substreams 0.7) |

> **Pin `prost` / `prost-types` to `0.13` — do not bump to 0.14.** `substreams 0.7` requires `prost ^0.13.3`; bumping puts two `prost` copies in the tree and produces exactly the link errors below. This pin is intentionally behind crates.io latest.

**Shared pitfalls:**
- `hex-literal` uses a hyphen in Cargo.toml
- Missing `prost-types = "0.13"` when generated `src/pb` references `Timestamp` / `Any`
- **Mix-major is not one failure mode.** Solana mismatched pairs (`substreams 0.6` + `substreams-solana 0.15`, or `0.7` + `0.14`) still **compile** but pull **two** `substreams` copies — check `cargo tree`, realign to a matched pair (`substreams-solana`). **Link / multiply-defined** errors are more often dual `prost` (0.13 vs 0.14) or `substreams-database-change` 4 on a `0.6` tree — realign then `rm -rf target && substreams build`
- WASM bindgen errors → `wasm/rust-v1+wasm-bindgen-shims` and/or `default-features = false` on `chrono` etc.

### Map / store shape (generic)

```rust
#[substreams::handlers::map]
pub fn map_events(/* chain block type */) -> Result<Events, Error> {
    // Transform one block → typed proto output
    Ok(Events::default())
}

#[substreams::handlers::store]
pub fn store_totals(events: Events, store: StoreAddInt64) {
    for item in events.items {
        store.add(0, &item.key, item.amount as i64);
    }
}
```

- **EVM block type + log decoding + eth_call + token stores:** `substreams-ethereum`
- **Solana block type + `walk_instructions` + Anchor/SPL:** `substreams-solana`
- Immutable per-key caches use **`set_if_not_exists` stores**, never a `HashMap` inside a map handler (RPC budget).

### Best Practices

* **Handle errors gracefully**: return `Result<T, Error>`; never panic in handlers
* **Log sparingly**: excessive logging costs performance (`substreams::log::info!` / `log::debug!` — there is no `warn!`)
* **Validate inputs**: check for null/empty data before processing
* **Test locally first**: always `substreams run` before deploying
* **Borrow, don't clone** (below)

## Performance

* **Add a `blockFilter`** to skip irrelevant blocks — by far the biggest cost lever (see above)
* **Minimize store size** — store only what you need
* **`--production-mode`** enables parallel execution for large ranges
* **Module granularity** — smaller, focused modules perform better; avoid deep dependency nesting

### Avoid cloning large structures

The chain `Block` and its transactions are large. Cloning them in a handler is the most common self-inflicted performance problem.

* ❌ **Never clone the `Block`** — copies every transaction with it.
* ❌ **Avoid cloning transactions** — extract the few fields you need instead.
* ✅ **Iterate by reference and borrow.** The chain crates' accessors already yield references — take them as `&T` and copy only the small owned values (a hash string, an amount) into your proto output.
* ✅ **Clone freely when it is genuinely small** — a `String` key reused across an inner loop is fine, and cloning a `Copy` primitive like `block.number` is a no-op you need not think about.

Chain-specific iteration (EVM `transactions()` / `logs_with_calls()`, Solana `walk_instructions()`) belongs to `substreams-ethereum` and `substreams-solana` — follow those skills for the correct loop shape, which also determines *which* records you see, not just how fast you see them.

> **Measuring:** `substreams run` executes your WASM **server-side**, so wall-clock time is dominated by network transfer and endpoint queueing, and a code change alters the module hash (cold cache) while the baseline ran warm. It is not a controlled A/B for handler CPU. To measure handler cost, benchmark at the WASM level — see `substreams-testing`.

## Common Patterns

[references/patterns.md](./references/patterns.md) — store aggregation, multi-module composition, parameterized modules, error handling.

* **EVM event extraction / ABI / eth_call** → `substreams-ethereum`
* **Solana instructions / SPL / Anchor** → `substreams-solana`
* **Database sink patterns** → `substreams-sql`

## graph_out Modules (The Graph / Subgraph Output)

> **Also load the `substreams-sink` skill** when building a `graph_out` module — it has the full working example.

| You want to write to | Output proto | Package |
|---|---|---|
| The Graph / subgraph | `EntityChanges` | `sf.substreams.sink.entity.v1` |
| Postgres / SQL | `DatabaseChanges` | `sf.substreams.sink.database.v1` |

**These are NOT interchangeable.** Using `DatabaseChanges` in a `graph_out` module (or vice versa) compiles but produces a pipeline that fails or emits garbage.

Do **not** rely on the `substreams-entity-change` crate for modern `substreams 0.7` pipelines. **v1** pins `prost ^0.11` (conflicts with prost 0.13); **v2.0.0** has `prost ^0.13` but still depends on **`substreams ^0.6`**, so it pulls a second `substreams` copy into a 0.7 tree. Inline the proto instead. Re-check [crates.io](https://crates.io/crates/substreams-entity-change) before changing this advice.

**`proto/entity.proto`** (exact package name required):
```proto
syntax = "proto3";

package sf.substreams.sink.entity.v1;

option go_package = "github.com/streamingfast/substreams-sink-entity-changes/pb/sf/substreams/sink/entity/v1;pbentity";

message EntityChanges {
  repeated EntityChange entity_changes = 5;   // NOT 1 — see the warning below
}

message EntityChange {
  string entity = 1;
  string id = 2;
  // Deprecated, this is not used within `graph-node`.
  uint64 ordinal = 3;
  enum Operation {
    OPERATION_UNSPECIFIED = 0;
    OPERATION_CREATE = 1;
    OPERATION_UPDATE = 2;
    OPERATION_DELETE = 3;
    OPERATION_FINAL = 4;
  }
  Operation operation = 4;
  repeated Field fields = 5;
}

message Value {
  oneof typed {
    int32 int32 = 1;
    string bigdecimal = 2;
    string bigint = 3;
    string string = 4;
    string bytes = 5;
    bool bool = 6;
    int64 timestamp = 7;
    Array array = 10;
  }
}

message Array {
  repeated Value value = 1;
}

message Field {
  string name = 1;
  optional Value new_value = 3;
  // Deprecated, this is not used within `graph-node`.
  optional Value old_value = 5;
}
```

> **Wire compatibility — field numbers are not cosmetic.** Copy this proto verbatim from the [canonical source](https://github.com/streamingfast/substreams-sink-entity-changes/blob/develop/proto/sf/substreams/sink/entity/v1/entity.proto). Package name, message names, field numbers, and field types must all match exactly.
>
> The failure mode is **silent**: `entity_changes` is tag **5**, not 1. Number it 1 and your module still compiles, still runs, and still emits data — but Graph Node reads tag 5, finds nothing, and ingests **zero entities** with no error. Likewise `Operation` must use the `OPERATION_*` names (a bare `UNSET`/`CREATE` enum changes the generated Rust variants), and `bytes` is a `string` field, not `bytes`.

The proven-working variant in [`examples/T3.2-cross-dex-volume/proto/entity.proto`](../../examples/T3.2-cross-dex-volume/proto/entity.proto) omits the deprecated `old_value` and unused `timestamp` — safe, since dropping unused optional fields does not shift the tags that matter.

```yaml
output:
  type: proto:sf.substreams.sink.entity.v1.EntityChanges
```

```rust
use crate::pb::sf::substreams::sink::entity::v1::{EntityChange, EntityChanges, Field};
use crate::pb::sf::substreams::sink::entity::v1::entity_change::Operation;
```

## Querying Chain Head Block

Requires data-plane auth (`substreams auth` / `SUBSTREAMS_API_TOKEN`).

```bash
# map_clocks emits sf.substreams.v1.Clock for every block; -s -1 starts at chain head.
# Do NOT combine -s -1 with a relative -t +N (the CLI requires an absolute start for +stop).
# Ctrl-C after the first line, or pipe through head.
substreams run https://spkg.io/v1/packages/common/v0.1.0 map_clocks \
  -s -1 --network mainnet -o jsonl
```

Read the first line for the head clock (`number` / hash). Default endpoint for a network: `substreams tools default-endpoint <network>`.

**Using firecore:** `firecore tools firehose-client <network-id-alias-or-host> -o json -- -1`

## Debugging

1. **Validate the graph**: `substreams graph` to visualize dependencies
2. **Test small**: run 100-1000 blocks, inspect output carefully
3. **Check logs**: WASM panics, protobuf decode errors
4. **Verify schema**: proto types match expected data
5. **Review inputs**: confirm input modules produce correct data
6. **Check `initialBlock`**: see the `initialBlock` guidance above — a genesis pin makes local runs impractical

### Troubleshooting

**Non-fatal package warnings** (`package.doc` deprecated, `README (package.doc) not found`, `URL (package.url) is not set`, `Description (package.description) is not set`): all four clear with a `package:` block that sets `url` + `description`, a sibling `README.md`, and no `doc:` field. Builds still succeed. See "Manifest Reference".

**Build fails**:
* `version "x.y.z" should match Semver` → add the `v` prefix to `package.version`
* `rustup target add wasm32-unknown-unknown`
* Ensure `buf` is installed (required for proto generation)
* Add `protobuf.excludePaths` with `sf/substreams` and `google` when importing spkgs
* Ensure the binary path in the manifest matches the build output

**Linking errors** ("symbol multiply defined", "failed to load bitcode") — almost always crate version mismatch:
* `rm -rf target && substreams build`
* Check for two `substreams` or two `prost` copies (`cargo tree -d`)
* Common link breakers: dual `prost` (pin 0.13), or `substreams 0.6` + `substreams-database-change 4` (needs 0.7)
* Dual `substreams` from a mismatched Solana pair often **still links** — fix the pins anyway (`substreams-solana`)

**Missing method on ABI-generated types** ("no method named `decode`"): add `use substreams_ethereum::Event;` and ensure `ethabi = "18"`.

**spkg import 404s**:
* Prefer **full URLs**: `https://spkg.io/v1/packages/<slug>/<version>` or `https://api.substreams.dev/v1/packages/<slug>/<version>`. Short `name@version` rewrites to `https://substreams.dev/v1/packages/...` and 404s (HTML site, not the package API).
* Use the `substreams-ethereum` spkg, NOT `sf-ethereum` (doesn't exist)
* Verify the release exists (registry `latestVersion`)
* Check the slug — registry slugs use hyphens (`ethereum-common`), while `name` may use underscores

**Empty output**:
* Confirm `initialBlock` is at or before the first relevant block
* Check the module isn't filtered out by an upstream index (`blockFilter` query mismatch — lowercase hex?)
* Verify input data exists in the block range

## Resources

* [Official Documentation](https://substreams.streamingfast.io)
* [Module Types](./references/module-types.md) · [Manifest Spec](./references/manifest-spec.md) · [Block Filtering](./references/block-filtering.md) · [Patterns](./references/patterns.md) · [Networks](./references/networks.md)
* **`substreams-ethereum`** — EVM contracts · **`substreams-solana`** — Solana programs
* [Discord](https://discord.gg/streamingfast) · [GitHub Issues](https://github.com/streamingfast/substreams/issues)
