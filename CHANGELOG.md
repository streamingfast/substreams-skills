# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

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