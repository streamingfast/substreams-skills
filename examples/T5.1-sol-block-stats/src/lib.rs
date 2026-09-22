use substreams_solana::pb::sf::solana::r#type::v1::Block;

mod pb {
    pub mod sol {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/sol.v1.mod.rs"));
        }
    }
}
use pb::sol::v1::BlockStats;

#[substreams::handlers::map]
fn map_block_stats(block: Block) -> Result<BlockStats, substreams::errors::Error> {
    let mut total = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    let mut compute_units = 0u64;

    for tx in &block.transactions {
        total += 1;
        let is_err = tx.meta.as_option().map(|m| m.err.is_set()).unwrap_or(false);
        if is_err {
            failed += 1;
        } else {
            success += 1;
            if let Some(meta) = tx.meta.as_option() {
                compute_units += meta.compute_units_consumed.unwrap_or(0);
            }
        }
    }

    Ok(BlockStats {
        slot: block.slot,
        parent_slot: block.parent_slot,
        total_transactions: total,
        successful_transactions: success,
        failed_transactions: failed,
        total_compute_units: compute_units,
    })
}
