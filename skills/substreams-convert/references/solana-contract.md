# Solana Contract → Substreams Conversion Guide

Load this file when building a Substreams pipeline from an existing Solana program (smart contract).

## Choose Your Conversion Path

Before writing any code, identify what source material you have:

| You have | Recommended path |
|---|---|
| **Anchor IDL** (`.json` file or on-chain IDL) | Use `substreams init` — it has built-in Solana IDL support and scaffolds the project for you automatically |
| **Rust source code** of the program | Follow the manual steps in this guide — read the source to extract discriminators, account layouts, and instruction args |

### Path A — Anchor IDL: Use `substreams init`

If the program has an Anchor IDL (either as a `.json` file or published on-chain), `substreams init` can scaffold the entire Substreams project from it:

```bash
substreams init
```

Select **Solana** as the chain and provide the program ID when prompted. The CLI will:
- Fetch the on-chain IDL (or accept a local `.json` file)
- Generate the protobuf schema from the IDL's instruction and account types
- Scaffold the Rust map handlers with correct discriminators pre-computed
- Generate `substreams.yaml` with the correct `network: solana` and `initialBlock`

After `substreams init` completes, review the generated code, set `initialBlock` to the program's deployment slot (not 0), then run `substreams build`.

> **If `substreams init` succeeds, you do not need to follow the manual steps below.** Return here only if the IDL is unavailable, the program is non-Anchor (native/raw), or you need to customise beyond what the scaffold produces.

### Path B — Rust Source: Manual Conversion

Use this path when:
- No IDL is available (non-Anchor or closed-source program)
- You are converting from Rust source code directly
- You need custom logic beyond what `substreams init` generates

Follow the step-by-step guide below.

---

## Conceptual Mapping

| Solana / Anchor Concept | Substreams Equivalent |
|---|---|
| Program ID | Address constant used for filtering in map module |
| IDL (Interface Definition Language) | Protobuf schema (`.proto`) |
| Instruction discriminator | `match` arm in Rust map handler |
| Instruction accounts | Decoded account slice from `ix_view.accounts()` |
| Instruction data | Parsed bytes from `ix_view.data()` |
| Event (Anchor `#[event]`) | Parsed from CPI log data |
| Transaction | Iterated via `block.transactions()` |
| Account state | Fetched on-demand or cached in a `store` |

## Prerequisites

```toml
# Cargo.toml — key dependencies for Solana Substreams
[dependencies]
substreams        = "0.6"          # Stay on 0.6.x — substreams-solana 0.14.x requires it
substreams-solana = "0.14.3"       # Block model + walk_instructions helper
bs58              = "0.4"          # base58 encode/decode for pubkeys
prost             = "0.13"
prost-types       = "0.13"

[build-dependencies]
prost-build = "0.13"

[profile.release]
lto       = true
opt-level = "s"
strip     = "debuginfo"
```

> **Version compatibility**: `substreams-solana 0.14.x` requires `substreams = "0.6"`. Check [crates.io/crates/substreams-solana](https://crates.io/crates/substreams-solana) for a newer release that may support `substreams = "0.7"` before assuming these exact versions. The compatibility matrix changes over time.
>
> **WARNING — do NOT mix `substreams = "0.7"` with `substreams-solana = "0.14"`**. This causes linker errors ("symbol multiply defined") at build time. The `substreams-dev` skill's Cargo.toml template recommends `0.7` for Ethereum — that does NOT apply to Solana. Always check the `substreams-solana` crate page for the currently required `substreams` version before starting.

## Step-by-Step Conversion

### 1. Identify the Program ID

Get the program's public key (base58 address). This is the Solana equivalent of an Ethereum contract address.

```rust
use substreams_solana::b58;

// Compile-time base58 decode — faster than runtime bs58::decode
const MY_PROGRAM: [u8; 32] = b58!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

// For multiple programs:
const RAYDIUM_AMM: [u8; 32] = b58!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const ORCA_WHIRLPOOL: [u8; 32] = b58!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
```

### 2. Convert the IDL Schema to Protobuf

**Before (Anchor IDL — instruction definition):**

```json
{
  "name": "swap",
  "accounts": [
    { "name": "tokenProgram", "isMut": false, "isSigner": false },
    { "name": "amm", "isMut": true, "isSigner": false },
    { "name": "ammAuthority", "isMut": false, "isSigner": false },
    { "name": "userSourceTokenAccount", "isMut": true, "isSigner": false },
    { "name": "userDestinationTokenAccount", "isMut": true, "isSigner": false },
    { "name": "userTransferAuthority", "isMut": false, "isSigner": true }
  ],
  "args": [
    { "name": "amountIn", "type": "u64" },
    { "name": "minimumAmountOut", "type": "u64" }
  ]
}
```

**After (Protobuf):**

```protobuf
syntax = "proto3";
package myproject.v1;

message Swap {
  string tx_signature         = 1;
  uint64 slot                 = 2;
  string amm                  = 3;  // pool/AMM address (base58)
  string user_source          = 4;  // source token account (base58)
  string user_destination     = 5;  // destination token account (base58)
  string user_authority       = 6;  // signer (base58)
  uint64 amount_in            = 7;
  uint64 minimum_amount_out   = 8;
}

message Swaps {
  repeated Swap swaps = 1;
}
```

### 3. Derive Instruction Discriminators

#### Anchor Programs (8-byte SHA256 discriminator)

Anchor prefixes every instruction with `sha256("global:<instruction_name>")[0..8]`:

```rust
// Add to Cargo.toml: sha2 = "0.10"
use sha2::{Digest, Sha256};

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}

// Pre-compute at compile time (recommended):
// $ echo -n "global:swap" | sha256sum | head -c 16  →  first 8 bytes
// Or use a const fn / build.rs to embed these as constants.
```

#### Native Programs (single-byte or custom discriminator)

For non-Anchor programs, check the program's source or documentation. Common patterns:

```rust
// SPL Token (single-byte discriminator in ix data[0]):
// 3  = Transfer
// 12 = TransferChecked
// 7  = MintTo
// 8  = Burn

// Custom protocol — read the source IDL or on-chain program binary
```

### 4. Implement the Rust Map Handler

**`src/lib.rs`:**

```rust
use substreams::errors::Error;
use substreams_solana::pb::sf::solana::r#type::v1::Block;
use substreams_solana::{b58, Block as BlockExt};

use crate::pb::myproject::v1::{Swap, Swaps};

const MY_PROGRAM: [u8; 32] = b58!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

// Anchor discriminator for "swap" instruction
const SWAP_DISC: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8]; // sha256("global:swap")[0..8]

#[substreams::handlers::map]
pub fn map_swaps(block: Block) -> Result<Swaps, Error> {
    let mut swaps = Swaps::default();

    for trx in block.transactions() {
        // block.transactions() already yields ONLY successful transactions.
        // To include failed transactions, iterate &block.transactions instead.
        let sig = trx.id();
        let sig = sig.as_str(); // borrow once per transaction, not cloned per instruction

        // ALWAYS use walk_instructions() — it yields top-level + all CPI inner instructions.
        // Using message.instructions directly misses ~90% of activity on aggregator-routed calls.
        for ix_view in trx.walk_instructions() {
            if ix_view.program_id() != MY_PROGRAM {
                continue;
            }

            let data = ix_view.data();

            // Verify Anchor discriminator
            if data.len() < 8 || &data[..8] != SWAP_DISC {
                continue;
            }

            // Parse instruction arguments (after the 8-byte discriminator)
            if data.len() < 24 {
                continue; // insufficient data
            }
            let amount_in = u64::from_le_bytes(data[8..16].try_into().unwrap());
            let minimum_amount_out = u64::from_le_bytes(data[16..24].try_into().unwrap());

            // Decode accounts (positional, matching IDL account list)
            let accounts = ix_view.accounts();
            if accounts.len() < 6 {
                continue;
            }

            swaps.swaps.push(Swap {
                tx_signature: sig.to_string(),
                slot: block.slot,
                amm: accounts[1].to_string(),                  // amm
                user_source: accounts[3].to_string(),          // userSourceTokenAccount
                user_destination: accounts[4].to_string(),     // userDestinationTokenAccount
                user_authority: accounts[5].to_string(),       // userTransferAuthority
                amount_in,
                minimum_amount_out,
            });
        }
    }

    Ok(swaps)
}
```

> **CRITICAL:** Always use `trx.walk_instructions()`, never `message.instructions`. The `walk_instructions()` method yields both top-level and inner (CPI) instructions. Using raw `message.instructions` silently misses 90%+ of DEX/protocol activity routed through aggregators.

### 5. Manifest Configuration

```yaml
specVersion: v0.1.0
package:
  name: my_solana_program
  version: v0.1.0
  url: https://github.com/myorg/my_solana_program   # set it — avoids package.url warning
  description: What this Solana program substreams indexes   # set it — avoids package.description warning
network: solana

protobuf:
  files:
    - myproject.proto
  importPaths:
    - ./proto

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/my_solana_program.wasm

modules:
  - name: map_swaps
    kind: map
    initialBlock: 320000000   # use the program's deployment slot
    inputs:
      - source: sf.solana.type.v1.Block
    output:
      type: proto:myproject.v1.Swaps
```

> **`initialBlock`**: Use the slot at which the program was deployed (or the earliest slot you care about). Setting `initialBlock: 0` forces a full chain backfill from Solana genesis — avoid this.

### 6. Anchor Events

Anchor emits events via CPI log data (base64-encoded). To parse them:

```rust
use base64::{engine::general_purpose, Engine as _};

// Anchor event log prefix: "Program data: <base64>"
// Events are emitted as CPI self-calls with log data.
// Substreams-Solana exposes log messages via trx meta:

for trx in block.transactions() {
    if let Some(meta) = &trx.meta() {
        for log in &meta.log_messages {
            if let Some(encoded) = log.strip_prefix("Program data: ") {
                if let Ok(bytes) = general_purpose::STANDARD.decode(encoded) {
                    // First 8 bytes = event discriminator
                    // sha256("event:<EventName>")[0..8]
                    if bytes.len() >= 8 && &bytes[..8] == MY_EVENT_DISC {
                        // Deserialize the rest as Borsh (Anchor default)
                    }
                }
            }
        }
    }
}
```

> **Note:** Anchor events require Borsh deserialization. Add `borsh = "0.10"` to `Cargo.toml` if needed. Borsh may not compile cleanly to `wasm32-unknown-unknown` in all toolchain configurations — if you encounter WASM bindgen import errors (e.g., `__wbindgen_placeholder__`), disable default features: `borsh = { version = "0.10", default-features = false }`. Test WASM compilation before committing.

### 7. Account State (Reading On-Chain Accounts)

Substreams does not have direct account state access like `AccountLoader` in Anchor. For account data:

**Option A — Parse account state from transaction `preTokenBalances` / `postTokenBalances`** (for SPL Token accounts):

```rust
if let Some(meta) = trx.meta() {
    for balance in &meta.pre_token_balances {
        let mint = &balance.mint;
        let owner = &balance.owner;
        let amount: u64 = balance.ui_token_amount
            .as_ref()
            .and_then(|a| a.amount.parse().ok())
            .unwrap_or(0);
        // ...
    }
}
```

**Option B — Decode `writable_accounts` from the transaction** (for custom account layouts):

```rust
// Transaction accounts accessible via:
let accounts = trx.resolved_accounts();  // Vec<Vec<u8>> — all accounts in order
// account at index 0 = accounts[0] (pubkey bytes)
```

> **Substreams does not expose raw account data bytes** from account state at a given slot. If you need account storage, you must:
> 1. Parse data fields emitted in transaction instruction data or logs, or
> 2. Use a `store` module to accumulate state derived from instructions.

### 8. Adding a Store for Aggregation

If the original program tracks cumulative state (e.g., total volume), use a `store` module:

```yaml
# substreams.yaml addition
  - name: store_volume
    kind: store
    initialBlock: 320000000
    updatePolicy: add
    valueType: int64
    inputs:
      - map: map_swaps
```

```rust
use substreams::store::{StoreAdd, StoreAddInt64};

#[substreams::handlers::store]
pub fn store_volume(swaps: Swaps, store: StoreAddInt64) {
    for swap in swaps.swaps {
        store.add(0, &swap.amm, swap.amount_in as i64);
    }
}
```

## SPL Token Special Cases

When the Solana contract interacts with SPL Token, filter by SPL Token program:

```rust
use substreams_solana::b58;

const SPL_TOKEN: [u8; 32] = b58!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const SPL_TOKEN_2022: [u8; 32] = b58!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

// Discriminator byte for Transfer (3) and TransferChecked (12):
// Use TransferChecked to filter by mint; plain Transfer has no mint field.
let data = ix_view.data();
match data.first().copied() {
    Some(12) if data.len() >= 10 => {
        // TransferChecked: [source, mint, dest, authority, ...signers]
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let mint = accounts.get(1).map(|a| a.to_string()).unwrap_or_default();
        if mint == USDC_MINT_STR {
            // process USDC transfer
        }
    }
    _ => {}
}
```

## Testing

```bash
# Build
substreams build

# Interactive visual debugger — best tool for inspecting module outputs during conversion
substreams gui ./substreams.yaml map_swaps \
  -s 320000000 -t +100 \
  --network solana

# Test with 100 slots starting from deployment
substreams run ./substreams.yaml map_swaps \
  -s 320000000 -t +100 \
  -o jsonl \
  --network solana

# Check for specific transaction
substreams run ./substreams.yaml map_swaps \
  -s <slot_with_known_swap> -t +1 \
  -o json \
  --network solana
```

## Common Pitfalls

| Pitfall | Solution |
|---|---|
| Using `message.instructions` | Always use `trx.walk_instructions()` — misses CPI inner instructions |
| Forgetting failed transactions | `block.transactions()` filters to **successful** txns only; use `&block.transactions` for all |
| Wrong discriminator bytes | Pre-compute with SHA256 and hardcode; verify against known transactions |
| Missing account index bounds check | Check `accounts.len() >= N` before indexing |
| `substreams = "0.7"` with `substreams-solana = "0.14"` | Use `substreams = "0.6"` for Solana |
| `initialBlock: 0` | Use program deployment slot — full Solana history is massive |
| Parsing Anchor events from `message.instructions` | Events are emitted in log messages, not instruction data |

## References

- [Substreams Solana documentation](https://substreams.streamingfast.io/tutorials/solana)
- [substreams-solana crate](https://crates.io/crates/substreams-solana)
- [Anchor IDL reference](https://www.anchor-lang.com/docs/idl)
- [Solana Program Library (SPL)](https://github.com/solana-labs/solana-program-library)
- [Solana Substreams reference file](../../substreams-dev/references/solana.md)
