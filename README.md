# Substreams Skills

Agent Skills for Substreams development - open-source expertise packages for AI assistants.

## What is this?

This is a **Claude Code Plugin** that provides AI assistants with expert knowledge about Substreams - a high-performance blockchain data indexing and transformation technology.

When installed, Claude gains deep expertise in:
- Building Substreams projects with `substreams.yaml` manifests
- Writing Rust modules (map, store, index types)
- Creating protobuf schemas for blockchain data
- Performance optimization and debugging

## Available Skills

### ✅ Substreams Development (`substreams-dev`)
Cross-cutting Substreams development for any chain:
- Creating and configuring `substreams.yaml` manifests
- Module graphs (map, store, index)
- Protobuf schema design and code generation
- Performance optimization and avoiding excessive cloning
- Debugging and skill routing to chain-specific packs

### ✅ Substreams Ethereum (`substreams-ethereum`)
Develop Substreams for **Ethereum and other EVM contracts**:
- ABI codegen (`build.rs`) and event decoding (`match_and_decode`)
- Raw topic0 / no-ABI decoding
- `eth_call` / `RpcBatch` and token-metadata stores
- Uniswap-style pool indexing patterns

### ✅ Substreams Solana (`substreams-solana`)
Develop Substreams for **Solana programs**:
- `walk_instructions()` (CPI-safe) vs top-level-only pitfalls
- Program filters with `b58!`, SPL Token, Anchor discriminators
- Instruction data + account layout parsing with or without IDL

### ✅ Substreams SQL (`substreams-sql`)
Expert knowledge for building SQL database sinks from Substreams data. Covers both mapping modes:
- **Database Changes (CDC)** — row-level INSERT/UPDATE/UPSERT/DELETE (**PostgreSQL only**)
- **From proto definition** — proto annotations → tables (**PostgreSQL and ClickHouse**; required for ClickHouse)
- **PostgreSQL** — schemas, indexes, delta aggregations, operational patterns
- **ClickHouse** — from-proto only; ORDER BY / PK rules, reserved names, analytics MVs
- **Schema Design** — best practices for blockchain data modeling

### ✅ Substreams Sink (`substreams-sink`)
Expert knowledge for consuming Substreams data in custom applications. Use when integrating Substreams outputs directly into Go, JavaScript, Python, or Rust code:
- **Go sink** — cursor management, reorg handling, gRPC streaming
- **JavaScript sink** — Node.js integration and event handling
- **Python / Rust** — SDK usage and production patterns

### ✅ Substreams Sink Deployment — Self-Managed (`substreams-sink-deploy-local`)
Expert operational guide for running Substreams sink binaries yourself (on your own machine, server, or container). Covers:
- **Sink selection** — decision tree for SQL, Files, PubSub, Webhook, ProtoJSON, Subgraph
- **SQL sink** — DSN formats, schema setup, cursor management, reorg handling (Postgres + ClickHouse)
- **Files sink** — CSV/Parquet to S3, GCS, or local storage
- **PubSub / Webhook** — event streaming and HTTP delivery
- **Production patterns** — backfill + live tailing, monitoring, restart safety
- **Common pitfalls** — wrong proto type, missing domain tables/schema.sql, PK mismatch, batch flush tuning

For a StreamingFast-hosted sink (no infrastructure to manage), see `substreams-hosted-sink`.

### ✅ Substreams Hosted Sink (`substreams-hosted-sink`)
Deploy and operate a Substreams sink on StreamingFast-hosted infrastructure — entirely through the Portal API `HostedService`, no binary to run. Covers:
- **Deployment type** — SQL sink (Postgres/ClickHouse) vs foundational store
- **Deploy flow** — build the `Deploy` request from a `.spkg` URL, provision the output DB (`DeployDatabase`), pick the `api_key_id` the sink streams under
- **Rollout & monitoring** — `GetDeploymentState` head block / lag, events, logs
- **Operate** — scale/pause (`SetReplica`), reconfigure (`UpdateDeploymentConfig`), reset and tear down — with confirmation on destructive actions
- **Auth** — device-code login via `portal-api-jwt`; endpoint reference in `portal-api`

### ✅ Portal API (`portal-api`)
Lets the assistant act on a StreamingFast Portal account by calling the Portal API directly — answering plain-language billing/usage/load questions **and** managing the full hosted-deployment lifecycle. The user does not have to write code — they just ask:
- "What's our current subscription?"
- "What's our usage this month / for API key X / over the last 30 days?"
- "Which service is driving most of our usage?"
- "What will our next bill look like?"
- "Who's connected right now / are we at capacity?"
- "Should we upgrade or downgrade based on our usage?"
- "Is my sink running / what's its head block and lag?"
- "Deploy this spkg / scale my sink to 3 replicas / undeploy X."

The skill covers six read-only billing/usage endpoints (`GetOrganizationSubscription`, `GetBillingDetails`, `GetUsageBilling`, `MultiServiceUsageSummaryByOrganization`, `UsageByOrganization`, `ActiveConnections`) plus the full `HostedService` surface (list/status/events/logs reads and deploy/scale/undeploy/reset/reconfigure mutations, the latter gated behind an explicit confirmation protocol). It documents how to authenticate via the `portal-api-jwt` device-code login (a short-lived `Authorization: Bearer` token), how to manage that session, a conversational playbook mapping common questions to endpoints, and a plan-change advice rubric. Proto fragments are inlined snapshots of the upstream API; when a call returns `invalid_argument` or a documented field is missing, treat the skill as potentially out of date and tell the user (there is no automatic schema-drift fetch).

### ✅ Portal API Auth (`portal-api-jwt`)
The interactive-login front-end for `portal-api` and the standard way to authenticate to the Portal API. Runs the OAuth 2.0 Device Authorization Grant (RFC 8628): the agent starts a device-code login, hands the user a URL + code to approve in their browser, **waits for the user to confirm they approved** (no background poll), then fetches a short-lived org-scoped access token plus a rotating refresh token. The result is an `Authorization: Bearer <token>` the `portal-api` skill sends on every call. Covers the full flow — start, user confirmation, capture, call, and refresh-with-rotation — plus token lifetimes and the secret-handling rules. (This is **Portal** admin auth only; streaming `substreams run` / sink auth is the separate `substreams auth` flow.)

### ✅ Substreams Testing (`substreams-testing`)
Expert knowledge for testing Substreams applications at all levels. Complete testing strategy:
- **Unit Testing** - Testing individual functions with real blockchain data
- **Integration Testing** - End-to-end workflows with real block processing
- **Performance Testing** - Benchmarking, memory profiling, and production mode validation
- **FireCore Tools** - Using Firehose, StreamingFast API, and testing utilities
- **CI/CD Integration** - Automated testing pipelines and regression detection

## Installation

### Claude Code (Recommended)

To install the plugin (which pulls the skills):

```bash
claude plugin marketplace add streamingfast/substreams-skills
claude plugin install substreams-dev@streamingfast-substreams
```

Or use the `/plugin` interactive flow directly within `claude`.

Validate that everything works properly by running `/skills` within `claude`, see example output:

```
...

Plugin skills (plugin)
portal-api · ~70 description tokens
portal-api-jwt · ~75 description tokens
substreams-dev · ~58 description tokens
substreams-ethereum · ~70 description tokens
substreams-solana · ~70 description tokens
substreams-sink · ~57 description tokens
substreams-sink-deploy-local · ~64 description tokens
substreams-hosted-sink · ~70 description tokens
substreams-sql · ~48 description tokens
substreams-testing · ~43 description tokens
```

**Alternative: Local Development**

Clone and load directly without installing:

```bash
git clone https://github.com/streamingfast/substreams-skills.git
claude --plugin-dir ./substreams-skills
```

### Cursor

Add the skill directory path in Cursor settings:
```
~/substreams-skills/skills/substreams-dev
```

### VS Code

VS Code 1.107+ supports Claude Skills (experimental feature):

1. Enable the experimental feature in settings
2. Add skill paths to your configuration
3. Skills will be available to Claude in VS Code

See [VS Code 1.107 release notes](https://code.visualstudio.com/updates/v1_107#_reuse-your-claude-skills-experimental) for details.

## Guides

Setup guides for various editors live under [`guides/`](./guides/).

## Examples & Evaluation

Real Substreams projects an agent built end-to-end from a natural-language prompt, using these skills:

- [`examples/`](./examples/) — 16 example directories (14 working case studies + 2 cautionary tales; Ethereum + Solana, single-map to multi-module, SQL sink, Anchor, no-ABI/no-IDL flows)
- [`EVAL.md`](./EVAL.md) — one-page summary of the test pass: 100% build/run, 12/14 byte-match correctness on the best trial

## Plugin Structure

```
substreams-skills/
├── .claude-plugin/
│   └── plugin.json          # Plugin metadata
└── skills/
    ├── portal-api/
    │   └── SKILL.md
    ├── portal-api-jwt/
    │   └── SKILL.md
    ├── substreams-dev/
    │   ├── SKILL.md          # Cross-cutting (manifest, modules, perf)
    │   └── references/
    ├── substreams-ethereum/
    │   ├── SKILL.md          # EVM contracts
    │   └── references/
    ├── substreams-solana/
    │   ├── SKILL.md          # Solana programs
    │   └── references/
    ├── substreams-sink/
    │   └── SKILL.md
    ├── substreams-sink-deploy-local/
    │   └── SKILL.md
    ├── substreams-hosted-sink/
    │   └── SKILL.md
    ├── substreams-sql/
    │   ├── SKILL.md
    │   └── references/
    └── substreams-testing/
        ├── SKILL.md
        └── references/
```

## Contributing

See [SKILL_DEVELOPMENT.md](./SKILL_DEVELOPMENT.md) for guidelines on creating new skills.

## Validation

Validate all skills against the specification:

```bash
npm run validate
```

## License

Apache 2.0 - See [LICENSE](./LICENSE)

## Resources

* [Substreams Documentation](https://substreams.streamingfast.io)
* [Claude Code Plugins](https://code.claude.com/docs/en/plugins)
* [StreamingFast Discord](https://discord.gg/streamingfast)
