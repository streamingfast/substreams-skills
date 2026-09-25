mod pb;

use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;

use pb::usdc::transfers::v1::{UsdcTransfer, UsdcTransfers};

// USDC contract address bytes (lowercase, no 0x prefix)
// 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
const USDC_ADDRESS: &str = "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

// ERC20 Transfer event topic0 (keccak256("Transfer(address,address,uint256)"))
// 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef
const TRANSFER_TOPIC: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Decodes a 32-byte ABI-encoded address topic to a hex string with 0x prefix.
fn decode_address(topic: &[u8]) -> String {
    // ABI-encoded address: 12 zero bytes of padding + 20 bytes address
    if topic.len() == 32 {
        format!("0x{}", hex::encode(&topic[12..32]))
    } else {
        format!("0x{}", hex::encode(topic))
    }
}

/// Decodes an ABI-encoded uint256 (big-endian bytes) to a decimal string.
fn decode_uint256(data: &[u8]) -> String {
    if data.is_empty() {
        return "0".to_string();
    }
    // We work with at most 32 bytes. Use u128 for amounts that fit; otherwise fallback to big math.
    // USDC has 6 decimals, max supply is ~44B * 1e6 ≈ 4.4e16, fits in u128 (max ~3.4e38).
    // For safety, support full 32-byte values by implementing minimal big-int to decimal.
    let bytes = if data.len() > 32 { &data[data.len() - 32..] } else { data };

    // Convert big-endian byte array to decimal string using simple long-division approach.
    // We represent the number as a vector of decimal digits (little-endian base-10).
    let mut digits: Vec<u8> = vec![0];

    for &byte in bytes {
        // Multiply existing digits by 256 and add new byte
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

    // digits are least-significant first; reverse and convert to chars
    digits.iter().rev().map(|d| (b'0' + d) as char).collect()
}

#[substreams::handlers::map]
pub fn map_usdc_transfers(block: Block) -> Result<UsdcTransfers, Error> {
    let mut transfers = UsdcTransfers::default();
    let block_number = block.number;

    for trx in block.transactions() {
        let tx_hash = format!("0x{}", hex::encode(&trx.hash));

        for (log, _call) in trx.logs_with_calls() {
            // Filter: must be emitted by USDC contract
            if hex::encode(&log.address) != USDC_ADDRESS {
                continue;
            }

            // ERC20 Transfer has exactly 3 topics: event sig + from + to
            if log.topics.len() < 3 {
                continue;
            }

            // Filter: topic0 must be the Transfer event signature
            if hex::encode(&log.topics[0]) != TRANSFER_TOPIC {
                continue;
            }

            let from = decode_address(&log.topics[1]);
            let to = decode_address(&log.topics[2]);
            // The transfer amount is the single data word (32 bytes, uint256)
            let amount = decode_uint256(&log.data);

            transfers.transfers.push(UsdcTransfer {
                tx_hash: tx_hash.clone(),
                log_index: log.index as u64,
                from,
                to,
                amount,
                block_number,
            });
        }
    }

    Ok(transfers)
}
