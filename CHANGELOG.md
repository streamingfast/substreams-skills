# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Fixed

- `substreams-solana` (v1.4.4): scaffolding so a skill-only agent can pack a Solana map — full manifest `protobuf:` + `binaries:` (pack fails without them), canonical `build.rs` + `mod pb` `OUT_DIR` include; label T5/T6 Cargo pins as **legacy** `0.6`+`0.14` (new work uses default `0.7`+`0.15`); Token-2022 same layout family as SPL Token but filter by program ID; mix-major table (mismatched Solana pairs dual-copy/still build vs prost/db-change link failures); index `Keys` snippets marked pattern-not-example-built; CPI eval note harmonized to ~10%; typed `Vec` skeleton + `TRACKED_POOL` define; slim hard-rule restatement in the critical-rules table.
- `substreams-dev` (v1.4.1): project layout no longer claims “Solana usually does not [add build.rs]” (every Solana example uses `prost_build`); shared pitfalls qualify mix-major by scenario instead of a blanket “link errors” claim.
- `substreams-hosted-sink` (v1.12.2): document the ops path — resolve deployment name → `deployment_id` via `ListDeployments` (never guess a slug), add the `SetReplica` request body, and point status/lag questions at `GetDeploymentState`; the quality gate now allows correcting false premises in the offer turn (no second question), rejects an option-2 answer that isn't `substreams run`, and clarifies that a local sink is legitimate work but not a substitute for the gate; fix duplicated pitfall numbering.
- `portal-api` (v1.16.3): verified the whole documented surface against the live server via gRPC reflection (`grpcurl admin.streamingfast.io:443`). Corrected `sf.hosted.common.v1.FoundationalStoreConfig` — `storage_class` is `reserved` upstream and `resources`/`ResourceConfig` no longer exist (pod CPU/memory/storage are not agent-configurable); `token`/`substreams_parameters` renumbered to 10/11; added the missing `FoundationalStoreMode mode = 12`. Documented that the server **silently discards unknown request fields** (a stale/misspelled field returns `200` and is ignored rather than erroring), that responses are camelCase-only while requests accept either casing, that `SetReplica` takes `sf.hosted.common.v1.ReplicaRequest` (body unchanged), and that `Deployment.deployment_request` is in fact returned (was commented out, contradicting the prose above it). Error section now covers the `details[].debug.fields` hint and the plain-text `404` for a mistyped method path. Added a reflection-based recipe for re-verifying the skill without repo access or a token.
- `portal-api-jwt` (v1.5.3): document `DeviceToken` behaviour verified by exercising the live endpoints — `SLOW_DOWN` rate-limits per device code at one call per `interval` (~5s) and *masks* the true status, so an approved login can read `SLOW_DOWN` and must never be reported as "not approved"; errors are a separate channel from the `status` enum (a never-issued device code → 400 `invalid_argument`, any `RefreshToken` failure → an indistinguishable 401 `unauthenticated`, so never retry it), while an aged-out code stays a 200 `DEVICE_TOKEN_STATUS_EXPIRED`. Also carry the camelCase-only finding into the surrounding prose, which still referenced `device_code` / `verification_uri_complete`.
- `portal-api-jwt` (v1.5.2): device-flow response examples now match the real wire shape — camelCase only (not mixed snake/camel), with `expiresIn`/`interval` as `int64`-encoded **JSON strings** (`"600"`, `"5"`) rather than numbers; note to coerce before arithmetic.
- `substreams-sql` (v1.4.0): **ClickHouse does support Database Changes** — removed the false “from-proto only / mode lock” that ran through `SKILL.md`, `database-changes.md` and `clickhouse-patterns.md`. Verified live: `setup` builds a `*db.ClickhouseDialect` and completes against ClickHouse 26.6.1. The real constraints are narrower and now documented: ClickHouse CDC is insert-only (`OnlyInserts()=true`), has no delta ops, and cannot do DB-side reorg management (`Revert()` errors, history path panics) so it needs `--undo-buffer-size > 0`. The actual rule is that **mutable state and delta aggregations require PostgreSQL + Database Changes**.
- `substreams-sql`: from-proto supports **at most one `primary_key: true` per table message** (`multiple field mark has primary keys are not supported`) — the ClickHouse PK/ORDER BY section previously prescribed multi-column PKs as the ✅ valid fix, which cannot build. The prefix rule now reduces to “`order_by_fields[0]` must be the PK field”.
- `substreams-sql`: reserved-column-names table was ~90% fabricated. Tested every entry on ClickHouse 26.6.1: only `index` breaks DDL; `keys`, `value`, `status`, `order`, `group`, `table` et al. are fine unquoted, and Postgres from-proto quotes identifiers so it never applied there.
- `substreams-sql`: correct live CLI facts, verified against the `v4.13.1` binary — `--batch-block-flush-interval` (default 1000) is `run`-only, from-proto uses `--block-batch-size` (default 25); DSN schemes are `[psql,postgres,clickhouse,parquet]` and `postgresql://` is rejected; ClickHouse HTTP 8123/8443 hard-rejected (use 9000/9440); DSN port defaults to 5432 even for ClickHouse.
- `substreams-sql`: from-proto cursors are **not** the `cursors` table — Postgres auto-creates `_cursor_`, ClickHouse stores the cursor in a **local file** (`cursor.txt`, `--clickhouse-cursor-file-path`), as is the schema hash; losing either silently breaks resume or triggers schema drift.
- `substreams-sql`: `sf.substreams.sink.sql.v1.Service` is deprecated → `sf.substreams.sink.sql.service.v1.Service`; `engine:` is inert for the CLI (dialect comes from the DSN); `sink:` block is optional for from-proto.
- `substreams-sql`: drop the “v4 proto FQN” mislabel — `sf.substreams.sink.database.v1.DatabaseChanges` has been stable since v1; only the Rust module path changed in v4. Fix delta-op trait guidance (`&String` has no `NumericAddable` impl → use `.as_str()`, not `.clone()`; `max`/`min` reject `String`/`&str`), note `add`/`sub` panic on non-numeric strings, `_version_`/`_deleted_` are ClickHouse-only, and document that from-proto’s migration path is an unimplemented stub (silent drift → `NO_SUCH_COLUMN_IN_TABLE`).
- `substreams-sql`: remove the broken “auto-refresh materialized view” trigger (`REFRESH … CONCURRENTLY` cannot run inside a transaction) which also contradicted the correct warning in the same file. Note upstream v4.13.1 bugs: `toStartOfMonth` silently ignored, `toYYYYDD` emits `toYYYYMMDD`.
- `substreams-sql`: slim to agent-sized — `SKILL.md` 1078 → 368 lines; delete `postgresql-patterns.md` (649) and `schema-patterns.md` (856), which never mentioned Substreams and carried ~13 SQL blocks that do not execute; rewrite `database-changes.md` and `clickhouse-patterns.md` around verified facts. 3790 → 845 lines total.
- README: SQL blurb no longer claims Database Changes is “PostgreSQL only” or that ClickHouse is from-proto-only.
- `substreams-dev` (v1.4.0): correct APIs and slim the skill. Embedded `EntityChanges` proto had **`entity_changes = 1` (must be `5`)** — a silent wire-format break that emits data Graph Node reads as zero entities; now copied verbatim from canonical (`OPERATION_*` enum names, `string bytes`, `optional old_value = 5`). Fix `- store: x, mode: deltas` (a YAML **parse error**) → block form at 5 sites; `StoreAppendBytes` → `StoreAppend<String>` (by value; appends semicolon-delimited strings, not concatenated bytes); remove non-existent `log::warn!` and `substreams run --debug`; drop the false “`imports:` REQUIRED for Ethereum chains” claim; database-changes spkg v3.0.0 → v4.0.0; network id `solana-mainnet-beta` → `solana`. Registry response shape corrected — proto3 **omits** `hasMore`/`packages`/`network`/`organization` rather than emptying them, `name` (underscores) ≠ `slug` (hyphens, used for URLs), and `network=mainnet` silently hides `ethereum-common`; drop unsourced rate-limit figures. Rewrite the cloning section: three snippets did not compile and two headline pitfalls taught the opposite (`.clone()` on a double reference costs nothing; the “✅ OK” clones were a redundant `String` clone and a no-op on `u64`); remove the unfounded “2-10x speedup” claim, since `substreams run` executes WASM server-side and the A/B was cache-confounded. Annotate the `prost 0.13` pin as deliberate (bumping to 0.14 breaks `substreams 0.7`). SKILL.md 1008 → ~580 lines; delete `references/solana.md` (~92% redundant with `substreams-solana`, and taught `CompiledInstruction` — the type it itself warned against); `patterns.md` 642 → 438 (drop broken/duplicate EVM section, dedupe `store_factory_contracts`, route SQL/testing to their skills).
- `substreams-solana` (v1.4.2): `is_failed` used `unwrap_or(true)`, treating a missing `meta` as a failed transaction; the golden `T5.1` example uses `unwrap_or(false)`.
- `substreams-sink` (v1.4.0): reviewed against upstream sources with every code block compiled/executed. **Inline `EntityChanges` proto was wire-incompatible** — `entity_changes` is field **5** (was 1), `old_value` is **5** (was 2), `Value.bytes` is a **`string`** (was proto `bytes`), `timestamp = 7` was missing, enum values are `OPERATION_*`; verified by round-trip: the old proto encodes fine and decodes to **zero entities** at a canonical consumer while `substreams run` still looks correct (which is why the eval could not catch it). Rust: `futures03` needs `package = "futures"` (no such crate exists — nothing built), `buf.gen.yaml` was missing the `neoeinstein-prost-crate` plugin that emits the `pb.rs` module tree, `SubstreamsEndpoint::new` takes a third `api_key` arg (`x-api-key`), token is sent **raw** (no `Bearer` prefix), `connect_lazy` + `max_decoding_message_size(10MB)`, and the cursor must advance **after** the `yield`. JS: documented `npm install` failed with `ERESOLVE` (pin connect 1.x); `fatalError` was unhandled and `Code.Internal` wrongly fatal, so module panics and bad tokens read as success (a bad token ends a `createGrpcTransport` stream with zero blocks and no error); `BlockRef.num` does not exist (use `.number`); `COULD_NOT_COMMIT_CURSOR` sentinel was never thrown. Python: `buf.gen.yaml` omitted the gRPC plugin so every `_pb2_grpc` import failed; oneof field is `session` (not `session_init`); `fatal_error` unhandled; `INTERNAL` is retryable; backoff never reset. Go: `sink.WithProductionMode()` does not exist (production is the default); unused imports broke compilation; missing `zap` and `lib/pq` imports; prefer `sink.ReadCursor`/`WriteCursor` (trims + atomic rename). Endpoints `optimism.` and `base.streamingfast.io` were NXDOMAIN (→ `mainnet.optimism.`/`base-mainnet.`), `bsc.` → `bnb.`; `--start-block :` is not valid syntax; `thegraph.market/supported-networks` 404s; finality delay is chain-specific (~13 min on Ethereum), not "~2-3 minutes".
- `substreams-sink-deploy-local` (v2.2.0): correct the `substreams-sink-files` section against the live binary (v2.3.1) — `run` takes `<manifest> [<module>] [<output>]` with endpoint as `-e` and range as `-s`/`-t` (the documented positional endpoint + `START:+N` form is rejected outright); encoders are `parquet`/`lines`/`protojson:.<field>[]` (no `csv`/`jsonl` encoder — CSV comes from `lines` with the module emitting CSV, and `lines` requires `sf.substreams.sink.files.v1.Lines`); cursor is `--state-store` default `./state.yaml`, not `_cursor.json` at the destination; `--buffer-max-size` is a bytes memory budget (default 64 MiB) so the old `=200000` example shrank it 300×; document `-c/--file-block-count` for file sizing. Also: name the real parallelism header `-H X-Substreams-Parallel-Workers`, and warn that the sink-sql DSN error advertises a `parquet` scheme that is an unimplemented leftover (panics in `from-proto`).
- `substreams-solana` (v1.4.3): every Rust snippet in the skill now compiles against the recommended `substreams 0.7.6` + `substreams-solana 0.15.0` pair (wasm32, release). The `index_from_block` index-module example did not build — `ix.accounts()` yields **owned** `Address` values and `Address` has no `Deref`, so `if *a == TRACKED` failed with `E0614`; drop the `*`. Correct the version-mixing claim: `0.6`+`0.15` (and `0.7`+`0.14`) do **not** produce "dual trees / link errors" — both compile and emit valid handler exports while cargo silently resolves two `substreams` copies, so a green build is not evidence the pins are right. Fix type comments (`ix.data()` is `&Vec<u8>`, `ix.program_id()` is `Address`, not `[u8; 32]`) and add troubleshooting rows for `E0614` and duplicate `substreams` in `cargo tree`.
- `substreams-hosted-sink` (v1.12.1): typo “Nerver” → “Never ask for a DSN”; add ClickHouse `Deploy` example; clarify `GoogleCloudSqlPrivate` is out of the standard agent path; curl examples use `$DEPLOYMENT_ID` from `CreateDeployment` (not invented slugs).
- README: hosted-sink blurb no longer says `DeployDatabase` provisions a DB or that agents pick `api_key_id` (attach user DB; key is auto-created).
- `portal-api` (v1.16.2): confirmation protocol no longer groups `DeployDatabase` with “provisions billable infrastructure” — attach/validate only; billable runner is `Deploy`.
- `substreams-solana` (v1.4.1): crate matrix updated to **`substreams 0.7` + `substreams-solana 0.15`** (legacy `0.6`+`0.14.x` only); clarify same-layout multi-disc messages (e.g. swap/swap_v2); index modules use `kind: blockIndex`; manifest example includes `package.url`/`description`.
- `substreams-dev` / legacy Solana notes / `substreams-convert`: align Solana Cargo pins with the matched-pair matrix (was incorrectly `0.6`+`0.15` or still on `0.14.3`).
- `substreams-sink` (v1.3.1): correct Go quickstart to monorepo API (`github.com/streamingfast/substreams/sink`, `NewFromViper` + `NewSinkerHandlers`); fix JS quickstart to official `streamBlocks` + `createGrpcTransport`; note entity-change v2 still pins `substreams ^0.6`; add skill routing vs sql/deploy-local/hosted-sink; align JS reference with gRPC transport.
- `substreams-sql` (v1.3.1): composite primary keys must use column/value tuples matching `schema.sql` (not string-concat keys); CDC manifest example uses a `v`-prefixed `package.version`; remove anti-pattern of module-level last-block skip (sink owns cursors); correct Postgres MV refresh guidance; note `--batch-block-flush-interval` for short smoke runs; align T2.3 database-changes import to v4.0.0 spkg with crate v4; README SQL blurb uses From-proto terminology. (The ClickHouse “mode lock” introduced here was itself incorrect and is reverted in v1.4.0 below.)
- `substreams-sink-deploy-local` (v2.1.0): correct live CLI facts — remove non-existent `generate`/`undo` and invented `--workers`; document `run <dsn> <manifest> [range]` (module from `sink:`), `tools cursor delete`, hand-written `schema.sql`, from-proto ops path, webhook/protojson signatures (`state.cursor`), and metrics default `localhost:9102`.
- `substreams-dev` (v1.3.1): short registry form `name@version` and `@latest` do not resolve via the CLI (rewrites to `substreams.dev/v1/packages/...` HTML 404) — prefer `https://spkg.io/v1/packages/<slug>/<version>` for `imports:`, `substreams run`/`info`, and head-block lookup; document relative `-t` + `-s -1` limitation.
- `substreams-dev`: package metadata example used unprefixed `version: 1.3.0` (must be `v`-prefixed); update Solana crate guidance to `0.15`+`substreams 0.7`; clarify `substreams-entity-change` v2 still pins `substreams ^0.6` so graph_out should keep inlining EntityChanges.
- `substreams-ethereum` (v1.0.2): manifest example was **unpackable** — add the required `protobuf:` and `binaries:` sections (all 13 example manifests carry both) plus `package.url`/`description`; note the wasm filename tracks the Cargo `[package]` name, not `package.name`.
- `substreams-ethereum` (v1.0.2): `ethabi` pin corrected `18` → `17`, and its role documented. `substreams-ethereum-core` pins `ethabi 17`, so `18` silently links a second `ethabi`/`ethereum-types`/`primitive-types` stack into the wasm and its `Token`/`ParamType` types are not interchangeable with the crate's. Also clarify `ethabi` is **required** (not optional) on the Abigen path — the generated module emits bare `ethabi::` paths into the user's crate and `substreams-ethereum` does not re-export it — while the hand-decode path (T6.1) needs no `ethabi` at all.
- `substreams-ethereum` (v1.0.2): reverse the `getrandom` + `substreams_ethereum::init!()` guidance — both were inherited from the crate's own stale docs and are **not** required on `substreams-ethereum 0.11`, which declares `getrandom = { features = ["custom"] }` for wasm itself. No example in this repo does either and all build. `init!()` registers an always-failing handler; it satisfies the linker, it does not provide randomness.
- `substreams-ethereum` (v1.0.2): store snippets omitted load-bearing imports — `StoreNew` (the `#[handlers::store]` macro emits a `::new()` call) and the `StoreSetIfNotExists`/`StoreGet` accessor traits; also note `StoreSetIfNotExistsProto` lives in `prelude`, not `store`.
- `substreams-ethereum` (v1.0.2): `rpc-and-tokens` `map_swaps` fragment had a bare `continue` outside any loop and an undefined `log` — replaced with a compiling loop; `abi-codegen` no longer calls `anyhow::anyhow!` (undeclared dependency) — use `substreams::errors::Error`.
- `substreams-ethereum` (v1.0.2): stop crediting T3.1 with `substreams::scalar::BigInt` — it squares `sqrtPriceX96` as a `num_bigint::BigUint`; document that `num-bigint`/`num-traits` are not transitively available through `substreams`.
- `substreams-ethereum` (v1.0.1): clarify that `Hex::encode` is unprefixed — emit addresses/tx as `0x` + lowercase hex; keep store keys consistent with lookups.
- `substreams-ethereum`: point T3.1 as the canonical `RpcBatch` + cache-store pattern.
- `examples/T2.2-univ2-swaps`: batch token0/token1 and ERC-20 metadata via `RpcBatch` instead of one `.call()` per field.
- `portal-api` (v1.16.1): upgrade/downgrade rubric compared `total_cents` to `base_price × 100` even though both fields are already integer cents — compare cents directly.
- `portal-api`: clarify `DeployDatabase` attaches an existing Postgres connection only (does not provision a DB; no `clickhouse_spec` — use Deploy `outputConfig.clickhouse`); note serverless DB cold-start retries on Deploy.
- `portal-api`: document production JSON quirks verified live — proto3 omits zeros (e.g. missing `plan_tier` ≈ Community), `int64`/`uint64` may be strings, empty billing/state objects are valid; deployment IDs are server UUIDs (not only a `dep` prefix).
- `portal-api-jwt` (v1.5.1): frontmatter `name` aligned with directory (`portal-api-jwt`, was `portal-api-auth`).
- README: remove incorrect claim of automatic schema-drift proto fetch; document that agents wait for user confirmation on device login (no background poll).

## [1.3.0](https://github.com/streamingfast/substreams-skills/releases/tag/v1.3.0)

### Fixed

- `substreams-dev` — document the non-fatal package metadata build warnings (`package.doc` deprecated, missing `package.url`/`package.description`, missing `README.md`) and how to avoid them.
- `substreams-dev`, `substreams-sql`, `substreams-bitcoin`, `substreams-convert` — manifest examples now set `package.url` + `package.description` and use a `v`-prefixed version so scaffolded packages build without metadata warnings.
### Added

- `substreams-dev` — registry package discovery via the new agentic search API (`GET /v1/registry/packages`), including `spkg` vs `reference` usage guidance.

## [1.2.0](https://github.com/streamingfast/substreams-skills/releases/tag/v1.2.0)

### Added

- New `substreams-bitcoin` skill — guidance for developing Substreams on Bitcoin.
- New `substreams-convert` skill — convert a subgraph to Substreams, referenced from `substreams-dev` (BLO-869).
- `substreams-dev` — README creation step in the development workflow (#14).
- `substreams-dev` — block/transaction filtering guidance (#12).
- `substreams-dev` — `substreams init` path for Solana IDL and Ethereum address-filtering guidance.
- SQL sink for storage documentation.

### Changed

- Prepped plugin manifests and enhanced `marketplace.json` descriptions for Anthropic marketplace submission (#8).

### Deprecated

- Marked `graph_out` as deprecated in favor of the SQL sink.

### Fixed

- `substreams-dev` — skill quality and correctness improvements.
- `substreams-convert` — improved Ethereum filter section and updated SQL skill link (#13).
- Corrected Substreams performance command examples and test instructions (#10).
- Fixed `ethabi` version and Solana paths in `SKILL.md`.

### Security

- Resolved Snyk security findings in skill examples (#9).

### Added

- `substreams-hosted-sink` / `portal-api`: mandatory pre-Deploy offer to test with `substreams run` so users can check data output quality before hosting.
- `substreams-hosted-sink` (v1.11.0): hardened **⛔ STOP** gate — session flag `OUTPUT_TEST_STATUS`, banned “ready to proceed to hosted deploy?” while unset, explicit order test → engine → deploy.
- `substreams-hosted-sink` (v1.12.0): quality gate is **`substreams run` command only** (stdout) — forbid local sink / ClickHouse-Postgres sync as the pre-deploy test; prefer handing the user the command.
- New `portal-api` skill — conversational billing/usage plus full hosted-deployment lifecycle over the Portal API.
- New `portal-api-jwt` skill — device-code (OAuth 2.0 Device Authorization Grant) login and token refresh; the standard way to authenticate to the Portal API.
- New `substreams-hosted-sink` skill — deploy and operate a Substreams sink on StreamingFast-hosted infrastructure entirely through the Portal API `HostedService`.
- `substreams-sql`: mandatory pre-flight choice tree — always offer `substreams-sink-sql` / `substreams-sink-kv` / `substreams-sink-files`; then PostgreSQL vs ClickHouse; then Database Changes vs From proto definition (Postgres only for the mode pair).
- New **`substreams-ethereum`** skill (v1.0.0) — EVM contract Substreams. Completes the chain-skill split started in `substreams-dev` v1.2.0, which routed EVM work here before the skill existed. Covers:
  - Pre-flight (one question per turn) and the ABI / Solidity-source / known-signature decoding decision.
  - `Abigen` + `build.rs` codegen and `match_and_decode`; the mandatory `use substreams_ethereum::Event;` import.
  - Raw topic0 decoding with indexed-vs-non-indexed rules; verified topic0 constants for ERC-20/721/1155, WETH and Uniswap V2/V3.
  - `eth_call` / `RpcBatch` with the `set_if_not_exists` cache-store pattern (T3.1: 41% → 100% correctness).
  - Hard rules: decode logs into typed protobuf, one message type per event, `uint256` → `BigInt` → decimal string.
  - `references/abi-codegen.md`, `references/rpc-and-tokens.md`, `references/common-contracts.md`.
- New **`substreams-solana`** skill — Solana program Substreams (`walk_instructions`, SPL, Anchor, no-IDL layouts).

### Changed

- Renamed `substreams-sink-deploy` → `substreams-sink-deploy-local` and scoped it to self-managed sink operation; hosted guidance now lives in `substreams-hosted-sink`.
- `substreams-sql` (v1.2.0): ClickHouse is **From proto definition only** (Database Changes not supported); added From proto definition guidance; renamed mode terminology to match product docs.
- `substreams-hosted-sink` (v1.7.0): hard limit that hosted SQL supports **only PostgreSQL and ClickHouse**; always offer engine choice; ClickHouse forbids Database Changes; reject other engines/KV/files as hosted targets.
- `substreams-dev` (v1.2.0): cross-cutting only — routes agents to `substreams-ethereum` / `substreams-solana` for chain-specific contract/program work; deep EVM eth_call and Solana sections removed from the main skill body.

## [1.1.0](https://github.com/streamingfast/substreams-skills/releases/tag/v1.1.0)

### Added

- New `substreams-sink-deploy` skill — run and deploy Substreams sinks (SQL, KV, Webhook, hosted). Includes DSN handling, batch flags, cursor management.
- 13 case-study example dirs (T1.1, T2.x–T5.x) demonstrating skill workflows.
- Test documentation scaffold — `EVAL.md` + `examples/` structure for evaluating skill outputs.

### Changed

- Applied F1–F37 patch series across skill files (#6) — correctness fixes.
- README — added `substreams-sink-deploy` to Available Skills and structure tree; clarified cursors pitfall wording.
- Bumped to newer `sfreleaser`.

### Fixed

- Sink DSN scheme: `postgresql://` → `psql://`.
- Sink CLI arg order.
- Cursor table auto-creation note.
- Webhook description, JSONL handling.
- Module names, unused deps, example fixes from PR review.
- Split DSN vars for sink vs psql client.
- Various Copilot-review fixes across skill frontmatter and docs.

## v1.0.3

### Added

- Updated `substreams-testing` skill with documentation for the new `substreams::testing` module introduced in `substreams-rs` 0.7.4+
  - Added documentation for `map!` macro for testing map handlers directly
  - Added documentation for `clock()` function for time-dependent tests
  - Added documentation for automatic `__impl_<name>` testable function generation
  - Added documentation for `#[substreams::handlers::map(no_testable)]` opt-out attribute
- Marked legacy wrapper function pattern (`_handler` functions) as deprecated in favor of `map!` macro

## v1.0.2

- Improved skills based on received feedback.