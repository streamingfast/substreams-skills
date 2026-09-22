use substreams::errors::Error;
use substreams_solana::b58;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

mod pb {
    pub mod pumpfun {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/pumpfun.v1.mod.rs"));
        }
    }
}
use pb::pumpfun::v1::{Launch, Launches};

/// Pump.fun program ID on Solana mainnet
const PUMPFUN: [u8; 32] = b58!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// Anchor discriminator for "create" instruction: sha256("global:create")[0..8]
const CREATE_DISC: [u8; 8] = [24, 30, 200, 40, 5, 28, 7, 119];

/// Pump.fun hardcoded initial virtual reserves (constants in the contract)
const INITIAL_VIRTUAL_SOL_RESERVES: &str = "30000000000";
const INITIAL_VIRTUAL_TOKEN_RESERVES: &str = "1073000000000000";

/// Parse a length-prefixed UTF-8 string from Anchor-encoded data.
/// Format: 4-byte LE u32 length + UTF-8 bytes.
fn parse_string(data: &[u8], offset: &mut usize) -> Option<String> {
    if *offset + 4 > data.len() {
        return None;
    }
    let len = u32::from_le_bytes(data[*offset..*offset + 4].try_into().ok()?) as usize;
    *offset += 4;
    if *offset + len > data.len() {
        return None;
    }
    let s = std::str::from_utf8(&data[*offset..*offset + len])
        .ok()?
        .to_string();
    *offset += len;
    Some(s)
}

#[substreams::handlers::map]
fn map_launches(block: Block) -> Result<Launches, Error> {
    let slot = block.slot;
    let mut launches = Vec::new();

    for trx in block.transactions() {
        // Skip failed transactions
        if let Some(meta) = trx.meta.as_option() {
            if meta.err.is_set() {
                continue;
            }
        }

        let sig = trx.id();

        for ix in trx.walk_instructions() {
            // Filter to pump.fun program only
            if ix.program_id() != PUMPFUN {
                continue;
            }

            let data = ix.data();
            if data.len() < 8 {
                continue;
            }

            // Check discriminator
            if data[..8] != CREATE_DISC {
                continue;
            }

            // Parse name, symbol, uri from data[8..]
            let mut offset = 8usize;
            let name = match parse_string(data, &mut offset) {
                Some(s) => s,
                None => continue,
            };
            let symbol = match parse_string(data, &mut offset) {
                Some(s) => s,
                None => continue,
            };
            // uri — parse but discard
            let _uri = parse_string(data, &mut offset);

            // Account layout:
            // [0] mint
            // [1] mint_authority
            // [2] bonding_curve
            // [3] associated_bonding_curve
            // [4] global
            // [5] mpl_token_metadata
            // [6] metadata
            // [7] user (creator)
            let accounts = ix.accounts();
            if accounts.len() < 8 {
                continue;
            }

            let mint = accounts[0].to_string();
            let creator = accounts[7].to_string();

            launches.push(Launch {
                slot,
                signature: sig.clone(),
                mint,
                name,
                symbol,
                creator,
                initial_virtual_sol_reserves: INITIAL_VIRTUAL_SOL_RESERVES.to_string(),
                initial_virtual_token_reserves: INITIAL_VIRTUAL_TOKEN_RESERVES.to_string(),
            });
        }
    }

    Ok(Launches { launches })
}
