# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Fixed

- `substreams-sink-deploy-local` (v2.1.0): correct live CLI facts — remove non-existent `generate`/`undo` and invented `--workers`; document `run <dsn> <manifest> [range]` (module from `sink:`), `tools cursor delete`, hand-written `schema.sql`, from-proto ops path, webhook/protojson signatures (`state.cursor`), and metrics default `localhost:9102`.
- `substreams-dev` (v1.3.1): short registry form `name@version` and `@latest` do not resolve via the CLI (rewrites to `substreams.dev/v1/packages/...` HTML 404) — prefer `https://spkg.io/v1/packages/<slug>/<version>` for `imports:`, `substreams run`/`info`, and head-block lookup; document relative `-t` + `-s -1` limitation.
- `substreams-dev`: package metadata example used unprefixed `version: 1.3.0` (must be `v`-prefixed); update Solana crate guidance to `0.15`; clarify `substreams-entity-change` v2 still pins `substreams ^0.6` so graph_out should keep inlining EntityChanges.
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