---
name: substreams-solana
description: >
  Develop Substreams for Solana programs — walk_instructions (CPI-safe), program/account
  filters, SPL Token, Anchor discriminators, IDL vs Rust-source instruction parsing.
  Always decode instruction data into typed structured protobuf objects (never raw bytes
  or JSON); one protobuf message type per instruction. Use when building Solana
  Substreams, decoding program instructions, Raydium/Pump.fun/SPL, or when the user
  mentions Solana programs, IDL, Anchor, or solana-common filters.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.4.2
  author: StreamingFast
  documentation: https://docs.substreams.dev/how-to-guides/develop-your-own-substreams/solana
---

# Substreams Solana Expert

Expert assistant for **Solana program** Substreams: instruction walking, decoding, and filters.
Cross-cutting manifests, stores, and performance live in **`substreams-dev`**. SQL module → **`substreams-sql`**. Self-managed sink ops → **`substreams-sink-deploy-local`**. StreamingFast-hosted SQL sink → **`substreams-hosted-sink`**.

Solana is **not** EVM. Do not use ABI/logs/topics patterns here. Decoding model = **instructions + accounts + discriminators**.

## Hard rule: decode instruction data into structured objects

**Always decode instruction `data` into typed fields.** Emitting only presence/program filters, raw bytes, hex, base58/base64 blobs, or a JSON string of the payload is **invalid** for protocol Substreams in this skill.

### Hard rule: one protobuf message type per instruction

**Each selected instruction gets its own dedicated protobuf message** with fields that match that instruction’s args + named accounts.

| Required | Forbidden |
|---|---|
| e.g. `message Swap { … }`, `message Deposit { … }` — one type per **distinct layout** | One generic `Event` / `Instruction` with optional fields for every ix |
| Distinct message per instruction the user asked for (different args/accounts) | Shared bag: `string instruction_name` + `map` / sparse optional args |
| Module output may wrap them: `repeated Swap swaps` **and** `repeated Deposit deposits`, or `oneof` of **those** messages | Single table of mixed rows with unused columns for other ixs |
| Parse args after the discriminator into **named typed values** | Leave `data` as `bytes` / hex / base64 |
| Map account metas to **named fields** (`pool`, `user`, `mint`, …) | Dump the whole instruction as JSON |
| | `instruction_data_json`, `raw_data`, `args_json`, `serde_json::to_string` |

**Same layout, multiple discriminators:** when two instructions share args + account layout (e.g. Raydium CLMM `swap` / `swap_v2` in T5.3), one message type is fine — match either disc, decode the same fields. Split only when layouts or account maps diverge.

**Why:** sinks, SQL (`schema.table` per message), and consumers need stable per-instruction schemas — not opaque blobs or a one-size-fits-all row.

**What “structured + one message per instruction” means in practice:**

```proto
// ✅ One message type per instruction
message Swap {
  uint64 slot = 1;
  string signature = 2;
  string pool = 3;
  string amount = 4;
  bool is_base_input = 5;
  string payer = 6;
}

message Deposit {
  uint64 slot = 1;
  string signature = 2;
  string user_wallet = 3;
  string sol_amount = 4;
}

// Output wrapper may hold several lists (or a oneof of Swap | Deposit)
message Events {
  repeated Swap swaps = 1;
  repeated Deposit deposits = 2;
}

// ❌ One mega-message for all instructions — forbidden
message GenericIx {
  string instruction = 1;   // "swap" | "deposit" | …
  string pool = 2;          // only set for swap
  string sol_amount = 3;    // only set for deposit
  bytes raw_data = 4;
}

// ❌ Opaque / JSON — forbidden
message BadSwap {
  bytes instruction_data = 1;
  string instruction_json = 2;
}
```

```rust
// ✅ Decode into typed locals, then into the proto object
let amount = u64::from_le_bytes(data[8..16].try_into().unwrap());
let is_base_input = data[40] != 0;
let pool = accounts[2].to_string();
out.push(Swap {
    slot,
    signature: sig.clone(),
    pool,
    amount: amount.to_string(),
    is_base_input,
    payer: accounts[0].to_string(),
});

// ❌ Never
// out.push(Event { data_json: serde_json::to_string(&data).unwrap(), .. });
// out.push(Event { raw_data: data.to_vec(), .. });
```

**Scope of the rule:**

* Applies to every **selected instruction** you emit (user allowlist from pre-flight).
* Discriminator matching alone is not enough — **args and relevant accounts must be decoded**.
* If a field is unavailable without fragile log parsing, set a typed zero/default and document it (see T5.3) — still do **not** replace decoding with JSON of the raw ix.
* Pure **block/tx stats** modules that never claim to decode a program (T5.1) are exempt; any module that filters on a program/instruction **must** decode.

**IDL note:** the Anchor **IDL file** may be JSON on disk — that is only the **source of layouts**. Runtime output is still typed protobuf objects, never “IDL JSON dump” of instruction data.

## Pre-flight (ALWAYS run first)

**Do not write Rust, protos, or manifests until the user has answered (or already stated) the items below.**

### How to ask (MANDATORY)

Same protocol as `substreams-dev` (and all Substreams skills):

1. **One question per turn** — never a multi-question dump.
2. **Offer choices** for every question (platform choice UI or numbered options).
3. **Always include** a final **“Other / enter a custom answer”** option.
4. Skip items the user already stated; wait for each answer before the next question.

### Step 1 — Scope inputs (ask missing items one at a time)

Walk this list **top to bottom**, one question per turn:

| Order | Question focus | Example choices (always end with custom) |
|---|---|---|
| 1 | **Network** | Solana mainnet · Solana devnet · **Other / custom** |
| 2 | **Program ID(s)** | Known protocols from [common-programs](./references/common-programs.md) · **Other / paste program ID** |
| 3 | **Instruction(s)** | Named ix presets for that program · All instructions · **Other / list discriminators or names** |
| 4 | **Account address(es) to track** | None (program-wide) · Common mint/pool · **Other / paste account(s)** |
| 5 | **Fields / output shape** | Suggested field set · Minimal (slot, sig, key accounts) · **Other / describe fields** |
| 6 | **Output / sink destination** | Full list below · **Other / custom** |
| 7 | **Slot range** | Suggested window · Recent head · **Other / enter slots** |

**“All instructions” + SQL/ClickHouse:** one protobuf message **and** typically one table per instruction (e.g. 30+ tables for Raydium CLMM). Prefer a **ladder** for first hosted deploy: high-value ixs (e.g. `swap` / `swap_v2`) → liquidity → full surface. If the user insists on all:

1. Use the **Anchor IDL** as the layout source and generate proto + decoders (do not hand-type 30 layouts).
2. Keep **one message type per instruction** (this skill’s hard rule).
3. Before ClickHouse: rename **reserved column names** (`index`, `keys`, …) per `substreams-sql`.
4. Test a short `substreams run` window before hosted Deploy (`substreams-hosted-sink`).

#### Output destination — its own turn

Ask only when destination is unknown. Offer **all** of these; last = custom:

| Choice | Follow-on |
|---|---|
| **`substreams run` only** | Verify with CLI/GUI first |
| **SQL — self-managed** | `substreams-sql` → `substreams-sink-deploy-local` |
| **SQL — StreamingFast hosted sink** | `substreams-sql` → **`substreams-hosted-sink`** |
| **The Graph / graph-out** | `substreams-sink` (EntityChanges) |
| **Custom app sink** | `substreams-sink` |
| **Not sure yet** | Start with run-only |
| **Other / enter a custom answer** | Free-text |

If the user says only “SQL”, ask **self-managed vs hosted** in a **separate** turn (plus custom). Hosted still needs a correct SQL module first.

If the user names a protocol without a program ID, propose the mainnet ID from [references/common-programs.md](./references/common-programs.md) as a **choice**, then still confirm instructions in a later turn.

### Step 2 — Parsing source decision (IDL vs Rust vs manual)

**One turn when ambiguous.** Prefer the highest-quality source available.

| Choice | When to use | How you decode |
|---|---|---|
| **Anchor IDL (JSON)** | User has IDL / `anchor build` / published IDL | Disc + args + accounts from IDL; optional `sol-anchor-beta` |
| **Program Rust source** | No IDL; Anchor source available | `sha256("global:<name>")` + layout from source |
| **Manual / known layout** | SPL Token or documented non-Anchor | Known discriminators + fixed LE fields |
| **Other / enter a custom answer** | User describes another source | Follow their layout; still structured decode |

**Decision rules:**

1. If user provides or can obtain an **IDL** → IDL path.
2. Else if **Rust instruction/context source** → Rust-source path.
3. Else if **SPL / well-known** → manual known layout.
4. Else → ask this question (choices above). Do **not** invent discriminators or account indexes.

Deep dive: [references/instruction-parsing.md](./references/instruction-parsing.md).

### Step 3 — Filter strategy

From program ID(s) + account address(es):

1. **In-module filter** — `if ix.program_id() != PROGRAM { continue; }` and optional account membership checks.
2. **Index module** (recommended when scanning large ranges) — emit keys like `program:<base58>`, `account:<base58>`, `ix:<name>` so consumers can skip empty slots.
3. **Foundational packages** — for “transactions touching program/account X” scaffolding, `substreams init` → **sol-transactions** uses cached [solana-common](https://substreams.dev/streamingfast/solana-common) filters (no voting noise when delayed from head). Still implement domain decoding in your map.

Details: [references/loops-and-filters.md](./references/loops-and-filters.md).

---

## Cargo.toml

**Prefer the current pair** (new projects). Do not mix majors across the table.

| Pair | When |
|---|---|
| `substreams = "0.7"` + `substreams-solana = "0.15"` | **Default** — `substreams-solana` 0.15 depends on substreams 0.7 |
| `substreams = "0.6"` + `substreams-solana = "0.14"` | Existing packages / examples that still pin 0.14.x |

```toml
[dependencies]
substreams = "0.7"
substreams-solana = "0.15"
bs58 = "0.4"
prost = "0.13"
prost-types = "0.13"
sha2 = "0.10"                  # Anchor discriminators when computing at runtime; prefer const bytes in hot paths

[build-dependencies]
prost-build = "0.13"

[lib]
crate-type = ["cdylib"]

[profile.release]
lto = true
opt-level = "s"
strip = "debuginfo"
```

**Do not** combine `substreams = "0.6"` with `substreams-solana = "0.15"` (or `0.7` with `0.14`) — dual trees / link errors. Re-check [crates.io/crates/substreams-solana](https://crates.io/crates/substreams-solana) before assuming these pins forever.

No `abi/` or `substreams-ethereum-abigen` for Solana projects.

## Manifest

```yaml
specVersion: v0.1.0
package:
  name: my_solana_substreams
  version: v0.1.0              # must be v-prefixed
  url: https://github.com/myorg/my-solana-substreams
  description: What this Solana substreams indexes
  # do not add package.doc — write README.md beside the manifest instead
network: solana
modules:
  - name: map_my_module
    kind: map
    initialBlock: 320000000   # pin to test/data range — not genesis
    inputs:
      - source: sf.solana.type.v1.Block
    output:
      type: proto:mypackage.v1.MyOutput
```

## Canonical loop: transactions + instructions

### Successful txs + all instructions (including CPI) — default for protocols

```rust
use substreams::errors::Error;
use substreams_solana::b58;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

const PROGRAM: [u8; 32] = b58!("YourProgramIdBase58........");

#[substreams::handlers::map]
fn map_events(block: Block) -> Result<MyEvents, Error> {
    let slot = block.slot;
    let mut out = Vec::new();

    // Successful transactions only
    for trx in block.transactions() {
        let sig = trx.id(); // base58 signature

        // Top-level AND inner (CPI) instructions — REQUIRED for DEX/protocol work
        for ix in trx.walk_instructions() {
            if ix.program_id() != PROGRAM {
                continue;
            }

            let data = ix.data();
            let accounts = ix.accounts(); // Vec<Address>, Display = base58

            // 1) Match discriminator for a user-selected instruction
            // 2) MUST decode data[disc..] into typed fields (not JSON, not raw bytes)
            // 3) MUST map accounts[i] to named fields from IDL / Accounts struct
            // 4) Optional: filter if tracked account is not in `accounts`
            // 5) Push a structured proto message (see Hard rule above)
            let _ = (slot, sig, data, accounts);
        }
    }

    Ok(MyEvents { items: out })
}
```

### Critical rules

| Rule | Why |
|---|---|
| **Always prefer `trx.walk_instructions()`** | Yields top-level **and** inner CPI instructions. Aggregator-routed DEX activity is almost always inner. |
| **Never use only `message.instructions`** | Top-level only. Eval impact: ~10% of Raydium CLMM swaps found vs 100% with `walk_instructions`. |
| **`block.transactions()` = successful only** | Failed txs skipped. Good for protocol events. |
| **Need failed txs / full counts?** | Iterate `&block.transactions` and check `meta.err` (stats pattern). |
| **Filter program ID first** | Cheapest reject before parsing data. |
| **Do not invent account indexes** | Take from IDL account metas or `#[derive(Accounts)]` field order (0-based). |
| **Decode ix data → structured objects** | Typed proto fields for args + named accounts. **No** raw data / hex / JSON string payloads. |
| **One proto message per instruction** | `Swap`, `Deposit`, … — never a single generic event for all ix types. |

Full patterns (failed txs, account filters, index modules): [references/loops-and-filters.md](./references/loops-and-filters.md).

## Instruction matching and decoding (after program filter)

Matching a discriminator is only step 1. **You must continue and decode the payload** into a structured output object (see [Hard rule](#hard-rule-decode-instruction-data-into-structured-objects)).

### Anchor (8-byte discriminator)

```rust
// Prefer compile-time constants (from IDL or precomputed sha256)
const SWAP_DISC: [u8; 8] = [/* from IDL or sha256("global:swap")[0..8] */];

if data.len() < 8 || data[..8] != SWAP_DISC {
    continue;
}
// payload starts at data[8..] — little-endian Anchor layout
// Decode each arg into typed locals, then fill a proto message — do not emit JSON of data[8..]
let amount = u64::from_le_bytes(data[8..16].try_into().unwrap());
// ...
```

Runtime helper (startup/tests only; prefer consts in maps):

```rust
use sha2::{Digest, Sha256};

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}
```

### SPL Token (1-byte discriminator)

- `3` = Transfer — accounts `[source, dest, authority, …]`; **no mint** → cannot filter by mint reliably.
- `12` = TransferChecked — accounts `[source, mint, dest, authority, …]`; **use this** when filtering a mint (e.g. USDC).

See [references/common-programs.md](./references/common-programs.md).

## Account filters (user-supplied addresses)

```rust
const TRACKED_MINT: [u8; 32] = b58!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

let accounts = ix.accounts();
// Example: TransferChecked mint at index 1 (from layout, not guessed)
if accounts.len() < 2 || accounts[1] != TRACKED_MINT {
    continue;
}

// Or: any involvement of a tracked pubkey
let touches = accounts.iter().any(|a| *a == TRACKED_POOL);
if !touches {
    continue;
}
```

When the user gives account addresses in pre-flight, wire them as `b58!` constants (or params) and filter **before** emitting.

## Project bootstrap options

| Goal | Approach |
|---|---|
| Full block, custom decode | Hand-written map on `sf.solana.type.v1.Block` (examples T5.x / T6.2) |
| Anchor + IDL | `substreams init` → **sol-anchor-beta**, point at IDL JSON |
| Tx filter by program/account | `substreams init` → **sol-transactions** (solana-common) |
| Simple program presence | `substreams init` → **sol-hello-world** |

After generators, still enforce pre-flight instruction list and account filters for production modules.

## Implementation checklist

1. Pre-flight complete (program, instructions, accounts, network, sink, slots).
2. Parsing path chosen (IDL / Rust source / manual).
3. `network: solana`, input `sf.solana.type.v1.Block`, crate versions as above.
4. Loop: `block.transactions()` + `walk_instructions()` + program ID filter.
5. Match only requested instructions (discriminators).
6. **Decode** instruction data into **typed structured fields** — **not** JSON strings, hex, or raw `bytes`.
7. **Define one protobuf message type per selected instruction** (e.g. `Swap`, `Deposit`); wrapper may use repeated fields or `oneof` of those types.
8. Map accounts to **named** fields from layout; apply account filters.
9. `initialBlock` = start of needed range; test with `substreams run -s <slot> -t +100 -o jsonl`.
10. Optional: index module keys for program/account/instruction.
11. Self-check: no generic mega-message for all ixs; no `*_json` / `raw_data` opaque blobs.
12. If sink is **ClickHouse / from-proto**: apply reserved-name renames and PK/ORDER BY rules from `substreams-sql` before first deploy.

## Examples in this repo

| Example | Pattern |
|---|---|
| T5.1 sol-block-stats | All txs including failed (`&block.transactions`) |
| T5.2 sol-usdc-transfers | SPL `TransferChecked` + mint filter |
| T5.3 sol-raydium-swaps | Anchor discs + `walk_instructions` |
| T5.4 sol-pumpfun-launches | Anchor `create` + string args + account indexes |
| T6.2 sol-marinade-no-idl | Discriminator + layout from Rust source only |

## Troubleshooting

| Symptom | Fix |
|---|---|
| Far fewer events than expected | You used top-level `message.instructions` — switch to `walk_instructions()` |
| Empty output | Wrong program ID, wrong disc, `initialBlock` past data, or filtering failed txs incorrectly |
| Mint filter never matches | Using SPL `Transfer` (3) instead of `TransferChecked` (12) |
| WASM / prost / link errors | Use a matched pair: `0.7`+`0.15` or `0.6`+`0.14.x` — never mix; `rm -rf target` and rebuild |
| Wrong account field | Re-check IDL account order or `Accounts` struct — do not guess indexes |
| Output is hex/base64/JSON of ix data | **Decode** into typed proto fields — see [Hard rule](#hard-rule-decode-instruction-data-into-structured-objects) |
| Only discriminator / program filter, no args | Incomplete — parse payload + named accounts for every emitted instruction |
| One `Event` for swap + deposit + create | Split into **one message type per instruction** |
| ClickHouse `SYNTAX_ERROR` on `index` / `keys` | Rename columns in proto (`config_index`, `account_keys`) — `substreams-sql` |

## Resources

* [Instruction parsing (IDL vs Rust)](./references/instruction-parsing.md)
* [Loops, filters, indexes](./references/loops-and-filters.md)
* [Common program IDs & SPL](./references/common-programs.md)
* [Solana docs](https://docs.substreams.dev/how-to-guides/develop-your-own-substreams/solana)
* [substreams-solana crate](https://github.com/streamingfast/substreams-solana)
* Cross-cutting: **`substreams-dev`** · SQL module: **`substreams-sql`** · Hosted sink: **`substreams-hosted-sink`** · Self-managed sink: **`substreams-sink-deploy-local`** · Testing: **`substreams-testing`**
