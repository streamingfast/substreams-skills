mod pb;

use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;

use pb::nft::mints::v1::{NftMint, NftMints};

// ERC721/ERC20 Transfer event topic0 (keccak256("Transfer(address,address,uint256)"))
// 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef
const TRANSFER_TOPIC: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

// Zero address (32 bytes) — ABI-encoded address with 12 bytes of zero padding
const ZERO_ADDRESS_TOPIC: [u8; 32] = [0u8; 32];

/// Decode a 32-byte ABI-encoded address topic to a 0x-prefixed hex string.
/// ABI encoding: 12 zero bytes of padding + 20 bytes address.
fn decode_address(topic: &[u8]) -> String {
    if topic.len() == 32 {
        format!("0x{}", hex::encode(&topic[12..32]))
    } else {
        format!("0x{}", hex::encode(topic))
    }
}

/// Decode a 32-byte big-endian uint256 to a decimal string.
/// tokenId can exceed u64, so we do full big-int conversion.
fn decode_uint256_decimal(data: &[u8]) -> String {
    if data.is_empty() {
        return "0".to_string();
    }
    let bytes = if data.len() > 32 { &data[data.len() - 32..] } else { data };

    // Convert big-endian byte array to decimal using long-multiplication
    let mut digits: Vec<u8> = vec![0];
    for &byte in bytes {
        let mut carry = byte as u32;
        for d in digits.iter_mut() {
            let val = (*d as u32) * 256 + carry;
            *d = (val % 10) as u8;
            carry = val / 10;
        }
        while carry > 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    digits.iter().rev().map(|d| (b'0' + d) as char).collect()
}

#[substreams::handlers::map]
pub fn map_nft_mints(block: Block) -> Result<NftMints, Error> {
    let mut result = NftMints::default();
    let block_number = block.number;

    for trx in block.transaction_traces.iter() {
        let tx_hash = format!("0x{}", hex::encode(&trx.hash));

        for log in trx.receipt.logs.iter() {
            // ERC721 Transfer has exactly 4 topics:
            //   topics[0] = event sig (Transfer)
            //   topics[1] = from (indexed address)
            //   topics[2] = to (indexed address)
            //   topics[3] = tokenId (indexed uint256)
            // ERC20 Transfer has 3 topics (value is NOT indexed, goes into data).
            // We MUST check topic count == 4 to avoid treating ERC20 transfers as ERC721.
            if log.topics.len() != 4 {
                continue;
            }

            // topic0 must be Transfer event signature
            if hex::encode(&log.topics[0]) != TRANSFER_TOPIC {
                continue;
            }

            // Mint condition: from == zero address (topics[1] is 32-byte ABI-encoded address)
            if log.topics[1] != ZERO_ADDRESS_TOPIC {
                continue;
            }

            let contract = format!("0x{}", hex::encode(&log.address));
            let minter = decode_address(&log.topics[2]);
            let token_id = decode_uint256_decimal(&log.topics[3]);

            result.mints.push(NftMint {
                contract,
                token_id,
                minter,
                tx_hash: tx_hash.clone(),
                log_index: log.index as u64,
                block_number,
            });
        }
    }

    Ok(result)
}
