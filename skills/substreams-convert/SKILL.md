---
name: substreams-convert
description: Expert knowledge for converting existing blockchain indexing projects into Substreams. Use when migrating a subgraph (The Graph) or a Solana program/contract into a Substreams module pipeline.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.6.0
  author: StreamingFast
  documentation: https://substreams.streamingfast.io
---

# Substreams Conversion Expert

Expert assistant for converting existing blockchain indexing projects — subgraphs and Solana contracts — into high-performance Substreams pipelines.

## Pre-flight (before writing code)

**Do not scaffold protos, Rust, or manifests until the user has answered (or already stated) these.** One question per turn when anything is missing; always offer an **"Other / custom"** option.

| Order | Focus | Typical choices |
|---|---|---|
| 1 | **Source** | Subgraph (`.yaml` + GraphQL + AS) · Solana Anchor IDL · Solana Rust source · **Other** |
| 2 | **Network / chain** | Ethereum mainnet · Base · Solana · **Other** |
| 3 | **Destination** | `substreams run` only · SQL self-managed · SQL hosted · Graph Node / EntityChanges (legacy) · **Other** |

Then hand off decoding and sink detail to the specialist skills — do not re-implement full tutorials here:

| Concern | Skill |
|---|---|
| EVM logs, Abigen, `logs_with_calls`, topic0 | **`substreams-ethereum`** |
| Solana `walk_instructions`, IDL/Anchor, SPL | **`substreams-solana`** |
| `db_out`, Postgres vs ClickHouse, `DatabaseChanges` | **`substreams-sql`** |
| Self-managed `substreams sink postgres`/`clickhouse` | **`substreams-sink-deploy-local`** |
| StreamingFast-hosted SQL sink | **`substreams-hosted-sink`** |
| EntityChanges / optional `graph_out` | **`substreams-sink`** (not the default greenfield path) |

**Mutable balances / current-state entities → PostgreSQL + Database Changes.** ClickHouse and from-proto are insert-only.

## Core Concepts

### Why Convert to Substreams?

Substreams offers several advantages over traditional indexers:

- **Parallelism**: Substreams processes historical data in parallel, providing up to 100× faster backfill than sequential indexers
- **Composability**: Modules from different packages can be combined, enabling code reuse across teams
- **Portability**: The same module logic can output to Postgres, ClickHouse, The Graph, Kafka, or any custom sink
- **Determinism**: WASM execution ensures identical outputs for identical inputs on any runner

### Supported Conversion Inputs

This skill covers two main conversion paths:

| Input | Reference File | Description |
|---|---|---|
| **Subgraph** (The Graph) | [references/subgraph.md](./references/subgraph.md) | Convert a GraphQL-schema subgraph with AssemblyScript handlers to Substreams |
| **Solana Contract** | [references/solana-contract.md](./references/solana-contract.md) | Build a Substreams pipeline from a Solana program — via `substreams init` (Anchor IDL) or manual conversion from Rust source |

## When to Use This Skill

Load this skill when the user wants to:

- Migrate a subgraph (`.yaml` + `schema.graphql` + AssemblyScript mappings) to Substreams
- Convert subgraph entity storage to a SQL sink (Postgres or ClickHouse) using `db_out`
- Convert a Solana program with an Anchor IDL into Substreams via `substreams init`
- Convert a Solana program from Rust source (no IDL) into Substreams Rust handlers manually
- Understand the conceptual mapping between subgraph entities and Substreams protobuf + SQL types
- Understand the conceptual mapping between Solana programs/instructions and Substreams modules

## Conversion Workflow Overview

### Step 1 — Pre-flight (source → network → destination)

Complete the [Pre-flight](#pre-flight-before-writing-code) table above. Do not skip destination: it drives whether you build `db_out`, stop at a domain map, or (rarely) keep EntityChanges.

### Step 2 — Load the relevant reference

- **Subgraph** → read [references/subgraph.md](./references/subgraph.md)
- **Solana Contract** → read [references/solana-contract.md](./references/solana-contract.md)

### Step 3 — Map the schema

Convert the source schema into Substreams protobuf types (`.proto` files).

### Step 4 — Implement Rust handlers

Translate the source handlers (AssemblyScript or Anchor/native Rust) into Substreams `map` and `store` modules. Prefer the loop shapes in **`substreams-ethereum`** / **`substreams-solana`** rather than inventing new ones.

### Step 5 — Configure the manifest

Wire modules together in `substreams.yaml`, referencing `initialBlock` from the original data source definition. For SQL: include standard `imports:` (database-changes + sql protodefs) and a `sink:` block — see the complete example in [references/subgraph.md](./references/subgraph.md).

### Step 6 — Build and verify

> **Also load `substreams-dev`** for Cargo.toml setup, build commands, `initialBlock` guidance, and general Rust module development patterns.
> **Also load `substreams-sql`** when the target output is a SQL sink (`db_out` → Postgres or ClickHouse).
> **Also load `substreams-sink-deploy-local`** when ready to run the converted substreams against a live sink (Postgres, ClickHouse, etc.) on your own infrastructure. For a StreamingFast-hosted sink, use `substreams-hosted-sink` instead.
> **Also load `substreams-sink`** only if you must keep Graph Node / EntityChanges (optional/legacy).

```bash
substreams build
substreams run ./substreams.yaml <output_module> -s <start_block> -t +100 -o jsonl
```

## Key Differences from Traditional Indexers

| Concept | Subgraph / Traditional | Substreams |
|---|---|---|
| Language | AssemblyScript / JavaScript | Rust (compiled to WASM) |
| Schema | GraphQL SDL | Protobuf (internal) + SQL schema (persistence) |
| Handler trigger | Event / call template | Block-level map module |
| State persistence | Entity store (auto-managed) | SQL sink via `db_out` + Postgres / ClickHouse |
| Intermediate state | Entity store | Explicit `store` modules (dynamic contract tracking) |
| Output | GraphQL API | SQL database, Kafka, custom — any sink |
| Reorg handling | Automatic (graph-node) | Cursor-based, explicit |
| Backfill speed | Sequential | Parallel (up to 100×) |

## Resources

- [Substreams Documentation](https://substreams.streamingfast.io)
- [Substreams GitHub](https://github.com/streamingfast/substreams)
- [substreams-sql skill](../substreams-sql/SKILL.md)
- [Subgraph Conversion Reference](./references/subgraph.md)
- [Solana Contract Conversion Reference](./references/solana-contract.md)
