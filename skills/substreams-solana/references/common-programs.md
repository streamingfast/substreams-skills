# Common Solana program IDs and layouts

Confirm mainnet vs devnet IDs with the user. Values below are **Solana mainnet** unless noted. Always re-verify against current protocol docs before production.

## System / token

| Program | Mainnet ID |
|---|---|
| System Program | `11111111111111111111111111111111` |
| SPL Token | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` |
| Token-2022 | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` |
| Associated Token Account | `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL` |

### Well-known mints (mainnet)

| Asset | Mint |
|---|---|
| USDC | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` |
| USDT | `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB` |
| wSOL | `So11111111111111111111111111111111111111112` |

## SPL Token / Token-2022 instruction cheatsheet

**Same layout family** for classic SPL Token and Token-2022 (Transfer / TransferChecked discriminators and account indexes). Always filter by **program ID** first — Token and Token-2022 are different programs:

| Program | Constant |
|---|---|
| SPL Token | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` |
| Token-2022 | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` |

```rust
const SPL_TOKEN: [u8; 32] = b58!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
// const TOKEN_2022: [u8; 32] = b58!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const USDC: [u8; 32] = b58!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

// if ix.program_id() != SPL_TOKEN { continue; }  // or TOKEN_2022 — do not mix silently
match data.first().copied() {
    Some(3) if data.len() >= 9 && accounts.len() >= 3 => {
        // Transfer — NO mint in accounts; skip if mint-filtering
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let (source, dest, auth) = (&accounts[0], &accounts[1], &accounts[2]);
    }
    Some(12) if data.len() >= 10 && accounts.len() >= 4 => {
        // TransferChecked — mint at index 1
        if accounts[1] != USDC { /* skip */ }
        let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let decimals = data[9];
        let (source, mint, dest, auth) =
            (&accounts[0], &accounts[1], &accounts[2], &accounts[3]);
    }
    _ => {}
}
```

## Protocol examples used in this repo

| Protocol | Program (mainnet) | Notes |
|---|---|---|
| Raydium CLMM | `CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK` | Anchor `swap` / `swap_v2` — T5.3 |
| Pump.fun | `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P` | Anchor `create` — T5.4 |
| Marinade Finance | `MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD` | Anchor `deposit` no-IDL — T6.2 |

Discriminators and account indexes for these examples live in the example `src/lib.rs` files — copy patterns, re-verify before production.

## Anchor discriminator reminder

```text
disc = first 8 bytes of sha256("global:<instruction_name>")
```

Instruction name = Anchor/Rust handler name (`swap`, `deposit`, `create`, …).

## Network in manifests

```yaml
network: solana   # mainnet routing for Substreams endpoints
```

For devnet/test endpoints, follow current [networks / endpoints](https://docs.substreams.dev) docs and confirm program IDs (often different from mainnet).

## Using IDs in pre-flight

When the user names a protocol but not an address:

1. Propose the mainnet ID from this table or protocol docs.
2. **Ask them to confirm** program ID, instructions, and any pool/mint filters.
3. Only then hardcode `b58!` constants.

Never silently swap a different program ID version (e.g. AMM vs CLMM) without confirmation.
