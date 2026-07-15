# Instruction parsing: IDL vs Rust source vs manual

Use this reference after the skill’s **Step 2** decision. Goal: stable discriminators, argument layouts, and account indexes — without inventing fields.

## Output contract: structured objects only

**Instruction data must be decoded into typed structure — never left opaque and never serialized as JSON.**

### One protobuf message per instruction (required)

Each selected instruction name/discriminator maps to **its own** `message` type:

| ✅ Do | ❌ Do not |
|---|---|
| `message Swap { … }` and `message Deposit { … }` | `message Event { string kind = 1; optional … }` covering all ixs |
| Output: `repeated Swap swaps` + `repeated Deposit deposits`, or `oneof event { Swap swap = 1; Deposit deposit = 2; }` | One row type with mostly null columns depending on kind |
| Field set matches that instruction’s args + accounts only | Shared sparse fields mixed across unrelated instructions |

For SQL from-proto sinks, this also means **one `schema.table` (or clear table mapping) per instruction message**, not one denormalized catch-all table.

| ✅ Do | ❌ Do not |
|---|---|
| Read LE integers, bools, length-prefixed strings, fixed pubkeys from `data[disc..]` into Rust locals | Emit `ix.data()` as `bytes`, hex, or base64 on the output message |
| Assign each value to a **named field on that instruction’s message** | Emit `instruction_json`, `args_json`, or any JSON string of the payload |
| Name accounts (`pool`, `mint`, `authority`) from layout indexes | Dump `accounts` as a single JSON array string without named fields |
| Prefer `string` for large chain integers when proto lacks uint128/256 | Use `serde_json` / `json!` as the primary encoding of instruction args |

The **IDL may be a JSON file** used only to discover layouts. That is input to the developer/agent — **not** the Substreams wire format. Runtime output = **protobuf object fields**, one message type per instruction.

```rust
// After disc match — always finish the decode:
let lamports = u64::from_le_bytes(data[8..16].try_into().unwrap());
let user = accounts[6].to_string();
deposits.push(Deposit {
    slot,
    signature: sig.clone(),
    user_wallet: user,
    sol_amount: lamports.to_string(),
});
```

## Path A — Anchor IDL (preferred when available)

### Where IDLs come from

* `anchor build` → `target/idl/<program>.json`
* Published npm / GitHub releases for the protocol
* On-chain IDL account (Anchor) if the program wrote one
* User-provided JSON

### What to extract from the IDL

For each instruction the user selected:

1. **Name** — e.g. `swap`, `deposit`, `create`
2. **Discriminator** — often present as 8 bytes in modern Anchor IDLs; if missing, compute `sha256("global:<name>")[0..8]`
3. **Args** — types and order → little-endian payload after the 8-byte disc
4. **Accounts** — ordered list → **0-based indexes** into `ix.accounts()`

### Generator shortcut

```bash
substreams init
# Protocol: Solana → generator: sol-anchor-beta → provide IDL path
```

Generated code still needs product filters (which instructions / accounts to keep). Prefer confirming the instruction allowlist with the user even when the IDL lists every ix.

### Typical IDL-driven match

```rust
const DEPOSIT_DISC: [u8; 8] = [/* from IDL or sha256("global:deposit") */];

if data.len() < 8 || data[..8] != DEPOSIT_DISC {
    continue;
}
// Parse args: data[8..] according to IDL arg types (u64 LE, bool, strings as 4-byte LE len + utf8, …)
// Account N: accounts[N] per IDL accounts array order
```

### Anchor string / bytes encoding (common)

* **String / bytes**: `u32` LE length + payload (see T5.4 Pump.fun `create` name/symbol/uri)
* **u64 / i64 / u128**: little-endian fixed width
* **bool**: 1 byte
* **Pubkey**: 32 bytes raw (in instruction data) — accounts are usually separate metas

## Path B — Program Rust source (no IDL)

Real-world case: open-source Anchor program, no published IDL (eval T6.2 Marinade).

### Discriminator

```text
sha256("global:<instruction_fn_name>")[0..8]
```

The name is the **Rust instruction function** name under `#[program]`, not a marketing name.

```rust
use sha2::{Digest, Sha256};

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}

// Prefer baking the result into a const once computed:
// const DEPOSIT_DISC: [u8; 8] = [0xf2, 0x23, 0xc6, 0x89, 0x52, 0xe1, 0xf2, 0xb6];
```

### Argument layout

From the instruction’s argument types / `AnchorDeserialize` structs:

* Same LE rules as IDL
* Payload starts at byte 8 after the discriminator

### Account indexes

From the `#[derive(Accounts)]` (or equivalent) struct field order — **declaration order is account index order** (0-based), including PDAs and program accounts listed there.

Example (conceptual):

```rust
// transfer_from is the 7th field → index 6
const TRANSFER_FROM_IDX: usize = 6;
let user = accounts.get(TRANSFER_FROM_IDX)?.to_string();
```

Document indexes with comments citing the source file so reviewers can verify.

### When source is incomplete

If only a fragment of the program is available, ask for:

* the `#[program]` module listing instruction names
* the `Accounts` struct for each requested instruction
* arg types for each requested instruction

Do not guess missing accounts or padding.

## Path C — Manual / non-Anchor

### SPL Token program

Program: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`

| Disc (u8) | Name | Data after disc | Accounts (prefix) |
|---|---|---|---|
| `3` | Transfer | `amount: u64` LE | source, dest, authority |
| `12` | TransferChecked | `amount: u64`, `decimals: u8` | source, **mint**, dest, authority |

**Mint filtering:** use **TransferChecked only**. Legacy Transfer has no mint in the account list; emitting Transfer while “filtering USDC” produces false positives unless you resolve token-account→mint via extra state (usually out of scope for a simple map).

### Other programs

Use protocol docs, explorers, or verified open-source clients. Record discriminator bytes and account maps in comments. Prefer user-supplied layout over scraping unreliable sources.

## Events / logs vs instruction data

Many protocols put **inputs** in instruction data and **outputs** (e.g. amounts out, ticks) in **program logs** or inner token transfers.

* Prefer fields available from **instruction data + accounts** when stable (T5.3 sets fragile log-derived fields to zero).
* Parsing Anchor events from raw log lines is fragile without the event disc + layout (often from the same IDL). Only do it when the user requires those fields and you have a verified layout.

## Anti-patterns

| Anti-pattern | Do instead |
|---|---|
| Invent discriminators from memory | IDL, `sha256("global:…")`, or known SPL tables |
| Guess account indexes (“pool is probably 0”) | IDL accounts list or `Accounts` struct order |
| Decode every IDL instruction by default | Process **only** user-selected instructions |
| Rely on top-level instructions for decoding | Always walk with `walk_instructions()` first |
| Mix EVM ABI tools | Solana has no ethabi path in this skill |
| Emit raw / hex / base64 instruction data | Decode into typed proto fields |
| Emit instruction args as a JSON string | Named scalar/message fields on the proto |
| Match disc only, skip arg parse | Always parse payload layout for emitted events |
| `map<string,string>` or free-form bag for args | Explicit fields (versioned when layout changes) |
| One generic proto for every instruction | **One message type per instruction** |

## Worked references in-repo

* **IDL-quality Anchor discs + data layout:** `examples/T5.3-sol-raydium-swaps`
* **No IDL, Rust-derived disc + accounts:** `examples/T6.2-sol-marinade-no-idl`
* **SPL manual:** `examples/T5.2-sol-usdc-transfers`
* **Anchor strings in data:** `examples/T5.4-sol-pumpfun-launches`
