# Evaluation Summary

These skills were tested by giving a Claude agent (`sonnet-4-6`) a plain-English prompt for each task and scoring the resulting Substreams project on three mechanical axes: **does it build, does it run, does its output match a golden reference**.

## Headline

- **14 tasks** covering Ethereum + Solana, single-map to multi-module pipelines, ABI-driven and source-only.
- **100% build success** across every trial.
- **100% run success** across every trial.
- **12 of 14 tasks** reach byte-level correctness against the golden on the best trial.
- **Sink-deploy task (T7.1)** reaches functional parity (537 rows match golden) using `substreams-sink-deploy-local`.

## Per-task results

| Task | Skill(s) | Build | Run | Correctness | Notes |
|---|---|---|---|---|---|
| T1.1 block stats | `substreams-dev` | ✅ | ✅ | 100% | |
| T1.2 USDC transfers | `substreams-dev` | ✅ | ✅ | 100% | |
| T2.1 NFT mints | `substreams-dev` | ✅ | ✅ | 100% | |
| T2.2 Uniswap V2 swaps | `substreams-dev` | ✅ | ✅ | 96% | MKR `bytes32 symbol()` edge case |
| T2.3 Postgres SQL sink | `substreams-dev`, `substreams-sql` | ✅ | ✅ | 100% | |
| T3.1 Uniswap V3 + USD price | `substreams-dev` | ✅ | ✅ | 100% | Required skill patch on `RpcBatch` token resolution |
| T3.2 Cross-DEX volume (graph_out) | `substreams-dev` | ✅ | ✅ | 100% | Embedded `EntityChanges` proto in skill |
| T5.1 Solana slot stats | `substreams-dev` | ✅ | ✅ | 100% | |
| T5.2 SPL USDC transfers | `substreams-dev` | ✅ | ✅ | 100% | |
| T5.3 Raydium CLMM swaps | `substreams-dev` | ✅ | ✅ | 100% | |
| T5.4 Pump.fun launches | `substreams-dev` | ✅ | ✅ | 100% | |
| T6.1 Uniswap V2, no ABI JSON | `substreams-dev` | ✅ | ✅ | 100% | Topic0 derived from Solidity source |
| T6.2 Marinade, no IDL | `substreams-dev` | ✅ | ✅ | 100% | Anchor discriminator derived from Rust source |
| T7.1 Sink deploy to Postgres | `substreams-sink-deploy-local` | ✅ | ✅ | 537/537 rows | DSN scheme + composite-PK gotchas surfaced and patched |

Detailed example output (manifest, Rust, proto) lives in [`examples/`](examples/).

## Known rough edges

- **Vague prompts produce confident guesses, not questions.** Skill text alone does not override model posture. T4.1 and T4.2 (intentionally vague) silent-shipped pipelines with hardcoded thresholds and token universes. **Be specific in prompts.** See [examples/T4.1-whale-activity](examples/T4.1-whale-activity/) and [examples/T4.2-uniswap-db](examples/T4.2-uniswap-db/).
- **Non-standard ERC20 tokens** (e.g. MKR returning `bytes32` from `symbol()`) cause field-decode failures. T2.2 hits this on ~4% of swaps. Functional, not silent — the agent emits the swap but `symbol` is empty.
- **Free-form proto field names.** When the prompt does not specify a schema, agents pick reasonable but inconsistent field names (`txCount` vs `transactionCount`, etc). For pipelines feeding downstream consumers, embed the proto schema in the prompt.
- **`substreams-entity-change` crate is unmaintained on the modern toolchain** (`prost = "0.11"` lock). The `substreams-dev` skill embeds the `EntityChanges` proto as a workaround for graph_out tasks until a 2.x release.

## How tasks were run

Each agent run started in an isolated worktree with only the prompt visible — no access to the golden, no access to other tasks. The agent loaded skills via the `substreams-skills` plugin and produced a Substreams project. After the agent finished, mechanical scoring ran:

- `substreams build` → **Build** axis
- `substreams run -s <range> -o jsonl` → **Run** axis
- Diff agent output against golden JSONL (after envelope strip + hex normalization) → **Correctness** axis

Soft axes (autonomy, code quality, efficiency) exist in the rubric but are not included here.

## Scope and caveats

- Single model (`sonnet-4-6`). Skill quality is largely model-orthogonal, but multi-model coverage is not part of this pass.
- Golden references for T4.x are intentionally absent — those tasks test clarification posture, not output correctness.
- The eval harness, full findings, and per-trial transcripts live in a private repo and are not reproduced here.
