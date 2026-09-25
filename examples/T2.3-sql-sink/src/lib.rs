mod pb {
    pub mod usdc {
        pub mod transfers {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/usdc.transfers.v1.mod.rs"));
            }
        }
    }
}

use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2::Block;
use substreams_database_change::pb::sf::substreams::sink::database::v1::DatabaseChanges;
use substreams_database_change::tables::Tables;

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
    let bytes = if data.len() > 32 { &data[data.len() - 32..] } else { data };

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

#[substreams::handlers::map]
pub fn db_out(transfers: UsdcTransfers) -> Result<DatabaseChanges, Error> {
    let mut tables = Tables::new();

    for transfer in transfers.transfers {
        // Composite primary key: (tx_hash, log_index) — matches schema.sql PRIMARY KEY
        let log_index_str = transfer.log_index.to_string();
        tables
            .create_row(
                "usdc_transfers",
                [("tx_hash", transfer.tx_hash.as_str()), ("log_index", log_index_str.as_str())],
            )
            .set("from_address", transfer.from)
            .set("to_address", transfer.to)
            .set("amount", transfer.amount)
            .set("block_number", transfer.block_number as i64);
    }

    Ok(tables.to_database_changes())
}
