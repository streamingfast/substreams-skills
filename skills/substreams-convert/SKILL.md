---
name: substreams-convert
description: Expert knowledge for converting existing blockchain indexing projects into Substreams. Use when migrating a subgraph (The Graph) or a Solana program/contract into a Substreams module pipeline.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.3.0
  author: StreamingFast
  documentation: https://substreams.streamingfast.io
---

# Substreams Conversion Expert

Expert assistant for converting existing blockchain indexing projects — subgraphs and Solana contracts — into high-performance Substreams pipelines.

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

### Step 1 — Identify the source

Ask the user which type of project they are converting:
- A subgraph (provides `subgraph.yaml`, `schema.graphql`, AssemblyScript mappings)?
- A Solana contract/program — do they have an **Anchor IDL** (use `substreams init`) or **Rust source** (manual conversion)?

### Step 2 — Load the relevant reference

- **Subgraph** → read [references/subgraph.md](./references/subgraph.md)
- **Solana Contract** → read [references/solana-contract.md](./references/solana-contract.md)

### Step 3 — Map the schema

Convert the source schema into Substreams protobuf types (`.proto` files).

### Step 4 — Implement Rust handlers

Translate the source handlers (AssemblyScript or Anchor/native Rust) into Substreams `map` and `store` modules.

### Step 5 — Configure the manifest

Wire modules together in `substreams.yaml`, referencing `initialBlock` from the original data source definition.

### Step 6 — Build and verify

> **Also load `substreams-dev`** for Cargo.toml setup, build commands, `initialBlock` guidance, and general Rust module development patterns.
> **Also load `substreams-sink`** or **`substreams-sql`** when the target output is a SQL sink (`db_out` → Postgres or ClickHouse).
> **Also load `substreams-sink-deploy-local`** when ready to run the converted substreams against a live sink (Postgres, ClickHouse, etc.) on your own infrastructure. For a StreamingFast-hosted sink, use `substreams-hosted-sink` instead.

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
