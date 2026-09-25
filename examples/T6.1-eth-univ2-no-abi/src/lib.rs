mod pb;

use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;

use pb::univ2::swaps::v1::{SwapEvent, SwapEvents};

// USDC-ETH Uniswap V2 pair address (lowercase, no 0x prefix)
// 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc
const PAIR_ADDRESS: &str = "b4e16d0168e52d35cacd2c6185b44281ec28c9dc";

// Swap event topic0: keccak256("Swap(address,uint256,uint256,uint256,uint256,address)")
// = 0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822
const SWAP_TOPIC: &str = "d78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";

/// Decodes a 32-byte ABI-encoded address topic to a hex string with 0x prefix.
/// ABI encoding: 12 zero bytes of padding + 20 bytes address.
fn decode_address(topic: &[u8]) -> String {
    if topic.len() == 32 {
        format!("0x{}", hex::encode(&topic[12..32]))
    } else {
        format!("0x{}", hex::encode(topic))
    }
}

/// Decodes an ABI-encoded uint256 (big-endian 32 bytes) to a decimal string.
fn decode_uint256(word: &[u8]) -> String {
    if word.is_empty() {
        return "0".to_string();
    }
    let bytes = if word.len() > 32 { &word[word.len() - 32..] } else { word };

    // Convert big-endian byte array to decimal using long multiplication.
    // digits stored least-significant first in base 10.
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
pub fn map_swaps(block: Block) -> Result<SwapEvents, Error> {
    let mut events = SwapEvents::default();
    let block_number = block.number;

    for trx in block.transactions() {
        let tx_hash = format!("0x{}", hex::encode(&trx.hash));

        for (log, _call) in trx.logs_with_calls() {
            // Filter: must be emitted by the USDC-ETH pair contract
            if hex::encode(&log.address) != PAIR_ADDRESS {
                continue;
            }

            // Swap event has 3 topics: event sig + indexed sender + indexed to
            if log.topics.len() < 3 {
                continue;
            }

            // Filter: topic0 must match Swap event signature
            if hex::encode(&log.topics[0]) != SWAP_TOPIC {
                continue;
            }

            let sender = decode_address(&log.topics[1]);
            let to = decode_address(&log.topics[2]);

            // Log data: 4 × 32-byte ABI words
            // [amount0In][amount1In][amount0Out][amount1Out]
            let data = &log.data;
            let amount0_in = if data.len() >= 32 {
                decode_uint256(&data[0..32])
            } else {
                "0".to_string()
            };
            let amount1_in = if data.len() >= 64 {
                decode_uint256(&data[32..64])
            } else {
                "0".to_string()
            };
            let amount0_out = if data.len() >= 96 {
                decode_uint256(&data[64..96])
            } else {
                "0".to_string()
            };
            let amount1_out = if data.len() >= 128 {
                decode_uint256(&data[96..128])
            } else {
                "0".to_string()
            };

            events.swaps.push(SwapEvent {
                tx_hash: tx_hash.clone(),
                log_index: log.index as u64,
                sender,
                to,
                amount0_in,
                amount1_in,
                amount0_out,
                amount1_out,
                block_number,
            });
        }
    }

    Ok(events)
}
