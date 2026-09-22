use substreams::errors::Error;
use substreams::scalar::BigInt;
use substreams_ethereum::pb::eth::v2::Block;

mod pb {
    pub mod eth {
        pub mod stats {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/eth.stats.v1.mod.rs"));
            }
        }
    }
}
use pb::eth::stats::v1::BlockStats;

#[substreams::handlers::map]
fn map_block_stats(block: Block) -> Result<BlockStats, Error> {
    let header = block
        .header
        .as_option()
        .ok_or_else(|| Error::msg("missing block header"))?;

    // Absent before London; a default would read as a real zero.
    let base_fee = header
        .base_fee_per_gas
        .as_option()
        .map(|bf| BigInt::from_unsigned_bytes_be(&bf.bytes).to_string())
        .unwrap_or_default();

    Ok(BlockStats {
        number: block.number,
        tx_count: block.transaction_traces.len() as u64,
        gas_used: header.gas_used,
        base_fee,
    })
}
