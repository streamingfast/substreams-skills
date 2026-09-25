use substreams::errors::Error;
use substreams_solana::b58;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

mod pb;
use pb::raydium::clmm::v1::{Swap, Swaps};

// Raydium CLMM program id
const RAYDIUM_CLMM: [u8; 32] = b58!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");

// Anchor discriminators (sha256("global:<name>")[0..8])
const SWAP_DISC: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
const SWAP_V2_DISC: [u8; 8] = [43, 4, 237, 11, 26, 201, 30, 98];

#[substreams::handlers::map]
fn map_swaps(block: Block) -> Result<Swaps, Error> {
    let slot = block.slot;
    let mut swaps: Vec<Swap> = Vec::new();

    for trx in block.transactions() {
        // block.transactions() returns only successful transactions
        let sig = trx.id();

        for ix in trx.walk_instructions() {
            if ix.program_id() != RAYDIUM_CLMM {
                continue;
            }

            let data = ix.data();
            // Minimum: 8-byte discriminator + 8-byte amount + 8-byte other_amount_threshold
            //          + 16-byte sqrt_price_limit_x64 + 1-byte is_base_input = 41 bytes
            if data.len() < 41 {
                continue;
            }

            let disc_ok = data[..8] == SWAP_DISC || data[..8] == SWAP_V2_DISC;
            if !disc_ok {
                continue;
            }

            // Parse instruction data layout (after 8-byte discriminator):
            //   amount:                 u64  LE  [8..16]
            //   other_amount_threshold: u64  LE  [16..24]
            //   sqrt_price_limit_x64:   u128 LE  [24..40]
            //   is_base_input:          bool     [40]
            let input_amount = u64::from_le_bytes(data[8..16].try_into().unwrap());
            // is_base_input = true means the user provides token0 (zero_for_one = true)
            let zero_for_one = data[40] != 0;

            // Pool address is account index 2
            let accounts = ix.accounts();
            if accounts.len() < 3 {
                continue;
            }
            let pool = accounts[2].to_string();

            // Fallback: output_amount and tick_after are not reliably parseable from
            // on-chain logs without knowing the exact runtime SwapEvent struct layout.
            // Per spec: set output_amount = "0", tick_after = 0 when log parsing is fragile.
            let output_amount: u64 = 0;
            let tick_after: i32 = 0;

            swaps.push(Swap {
                slot,
                signature: sig.clone(),
                pool,
                input_amount: input_amount.to_string(),
                output_amount: output_amount.to_string(),
                zero_for_one,
                tick_after,
            });
        }
    }

    Ok(Swaps { swaps })
}
