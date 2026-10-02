use substreams::errors::Error;
use substreams_solana::b58;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

mod pb;
use pb::marinade::deposit::v1::{Deposit, Deposits};

// Marinade Finance program ID
const MARINADE_PROGRAM: [u8; 32] = b58!("MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD");

// Anchor discriminator: sha256("global:deposit")[0..8]
// = [0xf2, 0x23, 0xc6, 0x89, 0x52, 0xe1, 0xf2, 0xb6]
const DEPOSIT_DISC: [u8; 8] = [0xf2, 0x23, 0xc6, 0x89, 0x52, 0xe1, 0xf2, 0xb6];

// Account index for transfer_from (the user signer) in the Deposit instruction.
// From context/deposit.rs Deposit<'info> struct (0-indexed):
//   0: state
//   1: msol_mint
//   2: liq_pool_sol_leg_pda
//   3: liq_pool_msol_leg
//   4: liq_pool_msol_leg_authority
//   5: reserve_pda
//   6: transfer_from   <-- user wallet (Signer)
//   7: mint_to
//   8: msol_mint_authority
//   9: system_program
//  10: token_program
const TRANSFER_FROM_IDX: usize = 6;

#[substreams::handlers::map]
fn map_deposits(block: Block) -> Result<Deposits, Error> {
    let slot = block.slot;
    let mut deposits: Vec<Deposit> = Vec::new();

    for trx in block.transactions() {
        // block.transactions() skips failed transactions automatically
        let sig = trx.id();

        for ix in trx.walk_instructions() {
            if ix.program_id() != MARINADE_PROGRAM {
                continue;
            }

            let data = ix.data();
            // Minimum: 8-byte discriminator + 8-byte u64 lamports = 16 bytes
            if data.len() < 16 {
                continue;
            }

            if data[..8] != DEPOSIT_DISC {
                continue;
            }

            // Parse lamports: bytes 8..16, little-endian u64
            let sol_amount = u64::from_le_bytes(data[8..16].try_into().unwrap());

            // Get user wallet (transfer_from) from accounts
            let accounts = ix.accounts();
            if accounts.len() <= TRANSFER_FROM_IDX {
                continue;
            }
            let user_wallet = accounts[TRANSFER_FROM_IDX].to_string();

            deposits.push(Deposit {
                slot,
                signature: sig.clone(),
                user_wallet,
                sol_amount: sol_amount.to_string(),
            });
        }
    }

    Ok(Deposits { deposits })
}
