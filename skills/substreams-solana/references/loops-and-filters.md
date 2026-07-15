# Loops, program/account filters, and indexes

## Transaction iteration

### Pattern A — All transactions (including failed)

Use for slot stats, failure rates, compute-unit totals.

```rust
for tx in &block.transactions {
    let is_failed = tx.meta.as_ref().map(|m| m.err.is_some()).unwrap_or(false);
    let compute = tx
        .meta
        .as_ref()
        .and_then(|m| m.compute_units_consumed)
        .unwrap_or(0);
    // signatures: tx.transaction.as_ref().map(|t| &t.signatures)
}
```

Example: `examples/T5.1-sol-block-stats`.

### Pattern B — Successful transactions only (default for protocol indexing)

```rust
for trx in block.transactions() {
    let sig: String = trx.id(); // base58
    // ...
}
```

`block.transactions()` skips failed transactions. Prefer this for swaps, transfers, deposits, launches.

## Instruction iteration

### Correct — `walk_instructions()`

```rust
for ix in trx.walk_instructions() {
    let program_id = ix.program_id(); // Address<'_> — compares directly against [u8; 32]
    let data = ix.data();             // &Vec<u8> (slices/indexes like &[u8])
    let accounts = ix.accounts();     // Vec<Address<'_>>, Display/to_string = base58
}
```

Walks **top-level and inner (CPI)** instructions. Required for almost all DeFi: routers and aggregators invoke target programs as **inner** instructions.

`Address` has **no** `Deref`. Compare it directly — `*addr` is a compile error unless
you are already holding a reference:

```rust
for a in ix.accounts() { if a == TRACKED { … } }          // a: Address    → no `*`
accounts.iter().any(|a| *a == TRACKED)                    // a: &Address   → `*` ok
```

### Wrong — compiled message instructions only

```rust
// ❌ Misses CPI / inner instructions — often 90%+ of protocol activity
for instr in message.instructions.iter() {
    // program_id_index + account_keys dance — do not do this for protocol detection
}
```

Eval note (T5.3 Raydium CLMM): top-level-only iteration found a small fraction of swaps; `walk_instructions()` found all.

There is essentially never a reason to hand-resolve `program_id_index` against `account_keys` for matching a program’s activity.

## Program ID filter

```rust
use substreams_solana::b58;

const RAYDIUM_CLMM: [u8; 32] = b58!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

for ix in trx.walk_instructions() {
    if ix.program_id() != RAYDIUM_CLMM {
        continue;
    }
    // decode…
}
```

* Use `b58!` for compile-time `[u8; 32]` constants (faster/cleaner than runtime decode in the hot path).
* Collect **program IDs in pre-flight** from the user (or confirm known protocol IDs).

## Account address filters

Collect **account addresses to track** in pre-flight (mints, pools, vaults, wallets). Apply them as early rejects.

### Fixed account index (from IDL / Accounts struct)

```rust
const USDC_MINT: [u8; 32] = b58!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

let accounts = ix.accounts();
// TransferChecked: mint at index 1
if accounts.len() < 2 || accounts[1] != USDC_MINT {
    continue;
}
```

### Any account involvement

```rust
const POOL: [u8; 32] = b58!("…");

let accounts = ix.accounts();
if !accounts.iter().any(|a| *a == POOL) {
    continue;
}
```

### Transaction-level account keys

When you need “did this tx touch account X at all” before walking instructions, inspect the message account keys / loaded addresses if required — but for instruction-centric pipelines, filtering on `ix.accounts()` after a program match is usually enough.

### Params vs constants

For reusable packages, optional module `params` can carry a base58 program or account list; for single-tenant indexers, `b58!` constants are fine and clearer.

## Index modules (block skip keys)

When scanning large slot ranges, emit an **index** so the runtime can skip empty blocks.

```rust
use substreams::pb::sf::substreams::index::v1::Keys;

// Handler is still #[map]; manifest kind is blockIndex.
#[substreams::handlers::map]
fn index_program_activity(events: MyEvents) -> Result<Keys, Error> {
    let mut keys = Keys::default();
    if events.swaps.is_empty() && events.deposits.is_empty() {
        return Ok(keys);
    }
    keys.keys.push("program:YourProgramId…".to_string());
    for swap in &events.swaps {
        keys.keys.push(format!("account:{}", swap.pool));
        keys.keys.push("ix:swap".to_string());
    }
    for dep in &events.deposits {
        keys.keys.push(format!("account:{}", dep.user_wallet));
        keys.keys.push("ix:deposit".to_string());
    }
    Ok(keys)
}
```

Manifest sketch:

```yaml
modules:
  - name: map_events
    kind: map
    inputs:
      - source: sf.solana.type.v1.Block
    output:
      type: proto:my.v1.Events

  - name: index_events
    kind: blockIndex          # not "index" — see substreams-dev block-filtering
    inputs:
      - map: map_events
    output:
      type: proto:sf.substreams.index.v1.Keys

  # A consumer must declare blockFilter to actually skip empty blocks:
  # - name: filtered_events
  #   kind: map
  #   blockFilter:
  #     module: index_events
  #     query:
  #       string: "program:YourProgramId…"
  #   inputs:
  #     - map: map_events
```

**Key naming conventions (suggested):**

| Key | Meaning |
|---|---|
| `program:<base58>` | Block has ix for this program |
| `account:<base58>` | Block touches this account in emitted data |
| `ix:<name>` | Block has this instruction type |
| `has_events` | Boolean presence flag |

Align keys with what consumers will query. Prefer stable base58 strings.

You can also build keys **while scanning the block** (even without a rich domain proto) if you only need presence filters:

```rust
#[substreams::handlers::map]
fn index_from_block(block: Block) -> Result<Keys, Error> {
    let mut keys = Keys::default();
    let mut seen_program = false;
    for trx in block.transactions() {
        for ix in trx.walk_instructions() {
            if ix.program_id() == PROGRAM {
                seen_program = true;
                for a in ix.accounts() {
                    if a == TRACKED {
                        keys.keys.push(format!("account:{}", a));
                    }
                }
            }
        }
    }
    if seen_program {
        keys.keys.push(format!("program:{}", bs58::encode(PROGRAM).into_string()));
    }
    Ok(keys)
}
```

## Foundational / init filters

`substreams init` on Solana offers:

| Generator | Role |
|---|---|
| **sol-hello-world** | Full block; filter one program; learn instruction walk |
| **sol-transactions** | Filter by program ID and/or account ID via **solana-common** foundational modules |
| **sol-anchor-beta** | IDL → decoded instructions/events |

Use foundational filters to **narrow input**, then apply domain decoding and the same pre-flight instruction allowlist.

Note: solana-common modules typically **exclude voting transactions**; for vote-inclusive full blocks use `sf.solana.type.v1.Block` directly. Delayed streams (>~1000 blocks from head) get cost/size benefits with those packages.

## Ordering inside a block

* Process instructions in walk order.
* Use `slot` + `signature` (+ optional instruction ordinal if you define one) for unique event IDs in sinks.
* Do not clone entire `Block` or transactions — extract fields only (`substreams-dev` performance rules).

## Checklist tying pre-flight → code

| User provided | Code artifact |
|---|---|
| Program ID(s) | `const …: [u8; 32] = b58!("…")` + early continue |
| Instruction names | Discriminator consts + match/allowlist |
| Account addresses | Mint/pool/wallet consts + index or membership checks |
| Large historical range | Index module keys for program/account/ix |
| “Any tx touching X” scaffold | sol-transactions / solana-common + custom map |
