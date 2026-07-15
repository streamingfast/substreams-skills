# Examples

Examples produced from natural-language prompts using these skills. Some folders contain end-to-end Substreams projects (buildable + runnable); others are writeups or operational notes documenting what the agent did, what worked, and what didn't.

All runs use `claude-sonnet-4-6`. For the buildable Substreams project examples, the code is reproducible: `cd` into the folder and `substreams build`.

## Ethereum

| Example | Skill(s) | Result |
|---|---|---|
| [T1.1 — Block stats](T1.1-block-stats/) | `substreams-dev` | Build · Run · 100% match |
| [T1.2 — USDC transfers](T1.2-usdc-transfers/) | `substreams-dev` | Build · Run · 100% match |
| [T2.1 — NFT mints](T2.1-nft-mints/) | `substreams-dev` | Build · Run · 100% match |
| [T2.2 — Uniswap V2 swaps + token metadata](T2.2-univ2-swaps/) | `substreams-dev` | Build · Run · 96% match (MKR `bytes32 symbol()` edge case) |
| [T2.3 — Postgres SQL sink](T2.3-sql-sink/) | `substreams-dev`, `substreams-sql` | Build · Run · 100% match |
| [T3.1 — Uniswap V3 swaps + USD price](T3.1-univ3-usd-price/) | `substreams-dev` | Build · Run · 100% match |
| [T3.2 — Cross-DEX volume aggregation (graph_out)](T3.2-cross-dex-volume/) | `substreams-dev` | Build · Run · 100% match |
| [T6.1 — Uniswap V2 swaps from Solidity source (no ABI)](T6.1-eth-univ2-no-abi/) | `substreams-dev` | Build · Run · 100% match |

## Solana

| Example | Skill(s) | Result |
|---|---|---|
| [T5.1 — Slot stats](T5.1-sol-block-stats/) | `substreams-dev` | Build · Run · 100% match |
| [T5.2 — SPL USDC transfers](T5.2-sol-usdc-transfers/) | `substreams-dev` | Build · Run · 100% match |
| [T5.3 — Raydium CLMM swaps](T5.3-sol-raydium-swaps/) | `substreams-dev` | Build · Run · 100% match |
| [T5.4 — Pump.fun launches (Anchor)](T5.4-sol-pumpfun-launches/) | `substreams-dev` | Build · Run · 100% match |
| [T6.2 — Marinade deposits from Anchor source (no IDL)](T6.2-sol-marinade-no-idl/) | `substreams-dev` | Build · Run · 100% match |

## Sink deployment

| Example | Skill(s) | Result |
|---|---|---|
| [T7.1 — Deploy SQL sink to Postgres](T7.1-sink-sql-deploy/) | `substreams-sink-deploy-local` | Sink installed, schema applied, 537 rows match golden |

## Cautionary tales (vague prompts)

| Example | What happened |
|---|---|
| [T4.1 — "Track whale activity"](T4.1-whale-activity/) | Agent shipped silently with hardcoded threshold, token universe, time range. Did not ask clarifying questions. |
| [T4.2 — "I want Uniswap data in my database"](T4.2-uniswap-db/) | Agent built full V3→Postgres pipeline without asking about chain, version, fields. |

These illustrate a known limitation: the skill text suggests asking for clarification on vague prompts, but model posture is dominant. **Vague prompts produce confident guesses, not questions.** Be specific.

## Related

- [`/EVAL.md`](../EVAL.md) — summary of the test pass against this skill set
