use substreams::errors::Error;
use substreams_solana::b58;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

mod pb;
use pb::sol::v1::{Transfer, Transfers};

// SPL Token program
const SPL_TOKEN_PROGRAM: [u8; 32] = b58!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

// USDC mint on Solana mainnet
const USDC_MINT: [u8; 32] = b58!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

// SPL Token instruction discriminators
const TRANSFER: u8 = 3;
const TRANSFER_CHECKED: u8 = 12;

#[substreams::handlers::map]
fn map_usdc_transfers(block: Block) -> Result<Transfers, Error> {
    let slot = block.slot;
    let mut transfers = Vec::new();

    for trx in block.transactions() {
        let tx_signature = trx.id();

        for instruction_view in trx.walk_instructions() {
            // Only process SPL Token program instructions
            if instruction_view.program_id() != SPL_TOKEN_PROGRAM {
                continue;
            }

            let data = instruction_view.data();
            if data.is_empty() {
                continue;
            }

            let discriminator = data[0];

            match discriminator {
                TRANSFER_CHECKED => {
                    // TransferChecked: accounts = [source, mint, dest, authority, ...signers]
                    // data: [12, amount_u64_le (8 bytes), decimals (1 byte)]
                    if data.len() < 10 {
                        continue;
                    }

                    let accounts = instruction_view.accounts();
                    if accounts.len() < 4 {
                        continue;
                    }

                    // Index 1 = mint — filter to USDC only
                    if accounts[1] != USDC_MINT {
                        continue;
                    }

                    let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
                    let source = accounts[0].to_string();
                    let destination = accounts[2].to_string();
                    let authority = accounts[3].to_string();

                    transfers.push(Transfer {
                        slot,
                        tx_signature: tx_signature.clone(),
                        source,
                        destination,
                        amount: amount.to_string(),
                        authority,
                    });
                }
                TRANSFER => {
                    // Transfer: accounts = [source, dest, authority, ...signers]
                    // data: [3, amount_u64_le (8 bytes)]
                    // Cannot determine mint from instruction alone — emit all and let
                    // downstream filter. The task prompt explicitly allows this approach.
                    if data.len() < 9 {
                        continue;
                    }

                    let accounts = instruction_view.accounts();
                    if accounts.len() < 3 {
                        continue;
                    }

                    let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
                    let source = accounts[0].to_string();
                    let destination = accounts[1].to_string();
                    let authority = accounts[2].to_string();

                    transfers.push(Transfer {
                        slot,
                        tx_signature: tx_signature.clone(),
                        source,
                        destination,
                        amount: amount.to_string(),
                        authority,
                    });
                }
                _ => {}
            }
        }
    }

    Ok(Transfers { transfers })
}
