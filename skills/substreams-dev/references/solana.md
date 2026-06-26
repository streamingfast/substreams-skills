# Solana Substreams

Solana uses a different block model, instruction paradigm, and account system than EVM chains. Do not apply Ethereum patterns here.

Load this file when working on Solana Substreams. For pure network-ID lookups (mainnet/devnet/accounts) see `networks.md`.

## Cargo.toml

```toml
[dependencies]
substreams = "0.6"             # Stay on 0.6.x — substreams-solana 0.14.x is not yet compatible with substreams 0.7; 0.5 is excluded because it pins prost 0.11
substreams-solana = "0.14.3"   # Block model + helpers
bs58 = "0.4"                   # pubkey/signature encode/decode
prost = "0.13"
prost-types = "0.13"

[build-dependencies]
prost-build = "0.13"

[profile.release]
lto = true
opt-level = "s"
strip = "debuginfo"
```

## Manifest

```yaml
specVersion: v0.1.0
package:
  name: my_solana_substreams
  version: v0.1.0
  url: https://github.com/myorg/my-solana-substreams   # set it — avoids package.url warning
  description: What this Solana substreams indexes        # set it — avoids package.description warning
  # NOTE: do not add a `doc:` field (deprecated) — write a README.md beside this manifest instead
network: solana
modules:
  - name: map_my_module
    kind: map
    inputs:
      - source: sf.solana.type.v1.Block
    output:
      type: proto:mypackage.v1.MyOutput
```

## Block access — two patterns

```rust
use substreams_solana::pb::sf::solana::r#type::v1::Block;
use substreams_solana::{b58, Block as BlockExt};  // helper trait

// Pattern A: iterate ALL transactions (including failed) — use for stats/counting
for tx in &block.transactions {
    let is_failed = tx.meta.as_ref().map(|m| m.err.is_some()).unwrap_or(true);
    let compute = tx.meta.as_ref()
        .and_then(|m| m.compute_units_consumed)
        .unwrap_or(0);
    // tx.transaction.as_ref().unwrap().signatures[0] = raw signature bytes
}

// Pattern B: iterate successful transactions with ergonomic helpers (most common)
// block.transactions() filters to successful only — use trx helpers for instruction walking
for trx in block.transactions() {
    let sig: String = trx.id();      // base58 transaction signature
    for ix_view in trx.walk_instructions() {
        // ix_view.program_id() → [u8; 32]
        // ix_view.data()       → &[u8]
        // ix_view.accounts()   → Vec<Address>  (each Address implements Display as base58)
    }
}
```

> **`block.transactions()` returns successful transactions only** (skips failed). For stats that need ALL transactions (including failed), use `&block.transactions` raw field (Pattern A).

> **`walk_instructions()` handles inner instructions automatically.** This is the key method — it yields both top-level instructions and all inner instructions (CPI calls). Never manually iterate `meta.inner_instructions` — use `walk_instructions()` instead.

> **CRITICAL — DO NOT iterate `message.instructions` for protocol detection (F37).**
> The compiled-instruction list (`tx.transaction.message.instructions`) holds **only top-level instructions**. On Solana, the vast majority of DEX/protocol activity (Raydium, Orca, Meteora, Jupiter routes, etc.) reaches your target program through CPI from an aggregator or router — those calls are **inner instructions** and are invisible to `message.instructions`.
>
> **Concrete impact:** A T5.3 Raydium CLMM eval found that agents using `for ix in message.instructions.iter()` detected only ~10% of swaps (45 of 99 blocks; 80 of 846 swaps). Agents using `for ix in trx.walk_instructions()` detected 100%. Same discriminators, same program ID — only iteration changed.
>
> ```rust
> // ❌ WRONG — misses 90% of Solana protocol activity
> for instr in tx.transaction.as_ref().unwrap().message.as_ref().unwrap().instructions.iter() {
>     if instr.program_id_index as usize ... { /* never sees aggregator-routed swaps */ }
> }
>
> // ✅ CORRECT — sees top-level + all inner CPI instructions
> for ix_view in trx.walk_instructions() {
>     if ix_view.program_id() != TARGET_PROGRAM { continue; }
>     // ...
> }
> ```
>
> If you find yourself writing `message.instructions` or resolving `program_id_index` against `account_keys` by hand, stop and rewrite with `walk_instructions()`. There is essentially never a reason to use the raw form.

## Program ID filtering with `b58!`

`b58!` is a compile-time macro that converts a base58 string to `[u8; 32]` — faster and cleaner than runtime decode:

```rust
use substreams_solana::b58;

const SPL_TOKEN: [u8; 32] = b58!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const USDC_MINT: [u8; 32] = b58!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

for ix_view in trx.walk_instructions() {
    if ix_view.program_id() != SPL_TOKEN { continue; }
    // ...
}
```

## SPL Token parsing

Instruction discriminator = first byte of `ix_view.data()`:
- `3` = Transfer: accounts `[source, dest, authority, ...signers]`, data = `[3u8, amount:u64 LE]`
- `12` = TransferChecked: accounts `[source, mint, dest, authority, ...signers]`, data = `[12u8, amount:u64 LE, decimals:u8]`

```rust
let data = ix_view.data();
let accounts = ix_view.accounts();   // Vec<Address>

match data.first().copied() {
    Some(3) if data.len() >= 9 && accounts.len() >= 3 => {
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let source = accounts[0].to_string();
        let dest   = accounts[1].to_string();
        let auth   = accounts[2].to_string();
    }
    Some(12) if data.len() >= 10 && accounts.len() >= 4 => {
        if accounts[1] != USDC_MINT { /* not USDC — skip */ }
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let source = accounts[0].to_string();
        let dest   = accounts[2].to_string();
        let auth   = accounts[3].to_string();
    }
    _ => {}
}
```

> **Filtering SPL transfers by mint? Use `TransferChecked` (12) only.** Its account layout includes the mint at index 1, so you can verify the token before emitting. Legacy `Transfer` (3) has no mint field — emitting it while filtering for a specific token (e.g. USDC) produces false positives for every SPL token, not just the one you want. Skip discriminator 3 unless you genuinely want all SPL transfers regardless of mint.

## Anchor discriminator

Anchor programs prefix instruction data with an 8-byte discriminator: `sha256("global:<instruction_name>")[0..8]`

```rust
// Add to Cargo.toml: sha2 = "0.10"
use sha2::{Digest, Sha256};
use substreams_solana::pb::sf::solana::r#type::v1::CompiledInstruction;

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}

fn is_anchor_ix(ix: &CompiledInstruction, disc: &[u8; 8]) -> bool {
    ix.data.len() >= 8 && &ix.data[..8] == disc
}

// Usage:
let swap_disc = anchor_discriminator("swap");
if is_anchor_ix(ix, &swap_disc) { /* it's a swap */ }
```

## Block-level fields

```rust
block.slot          // u64 — slot number
block.parent_slot   // u64 — parent slot

// Failed transaction check:
tx.meta.as_ref().map(|m| m.err.is_some()).unwrap_or(true)  // true = failed

// Compute units consumed (successful txns only):
meta.compute_units_consumed  // Option<u64> (may be None on older slots)
```

## Running against Solana

```bash
# Uses network: solana in substreams.yaml to route to mainnet endpoint
substreams run ./substreams.yaml map_my_module -s 320000000 -t +100 -o jsonl
```
