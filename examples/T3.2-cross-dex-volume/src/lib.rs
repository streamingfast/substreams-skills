mod abi;
mod pb;

use std::collections::HashSet;

use num_bigint::BigUint;
use num_traits::Zero;
use substreams::errors::Error;
use substreams::prelude::StoreSetIfNotExistsProto;
use substreams::scalar::BigDecimal;
use substreams::store::{
    DeltaBigDecimal, Deltas, StoreAdd, StoreAddBigDecimal, StoreGet, StoreGetProto, StoreNew,
    StoreSetIfNotExists,
};
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::Event;

use pb::dex::volume::v1::{PoolTokenEntry, PoolTokenPairs, SwapVolume, SwapVolumes, TokenPair};
use pb::sf::substreams::sink::entity::v1::{
    entity_change::Operation, EntityChange, EntityChanges, Field, Value,
};
use pb::sf::substreams::sink::entity::v1::value::Typed;

// ─── Event topic0 constants ───────────────────────────────────────────────────

/// Uniswap V2 Swap: keccak256("Swap(address,uint256,uint256,uint256,uint256,address)")
const V2_SWAP_TOPIC: [u8; 32] =
    hex_literal::hex!("d78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822");

/// Uniswap V3 Swap: keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)")
const V3_SWAP_TOPIC: [u8; 32] =
    hex_literal::hex!("c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67");

// ─── Store delta operation from substreams ────────────────────────────────────
use substreams::pb::substreams::store_delta::Operation as DeltaOperation;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn addr_hex(addr: &[u8]) -> String {
    format!("0x{}", hex::encode(addr))
}

/// Store key for token metadata: "<protocol>:<pool_address>"
fn token_store_key(protocol: &str, pool: &str) -> String {
    format!("{}:{}", protocol, pool)
}

/// Entity ID and store key for volume: "<protocol>:<pool_address>:<day_bucket>"
fn volume_key(protocol: &str, pool: &str, day: u64) -> String {
    format!("{}:{}:{}", protocol, pool, day)
}

/// Fetch ERC-20 symbol + decimals via RPC batch.
fn fetch_token_info(addr: &[u8]) -> (String, u32) {
    use substreams_ethereum::rpc::RpcBatch;

    let result = RpcBatch::new()
        .add(abi::erc20::functions::Symbol {}, addr.to_vec())
        .add(abi::erc20::functions::Decimals {}, addr.to_vec())
        .execute();

    match result {
        Ok(resp) => {
            let symbol =
                RpcBatch::decode::<_, abi::erc20::functions::Symbol>(&resp.responses[0])
                    .unwrap_or_else(|| "UNKNOWN".to_string());
            let decimals =
                RpcBatch::decode::<_, abi::erc20::functions::Decimals>(&resp.responses[1])
                    .map(|d| d.to_u64() as u32)
                    .unwrap_or(18u32);
            (symbol, decimals)
        }
        Err(_) => ("UNKNOWN".to_string(), 18),
    }
}

/// Fetch pool token0+token1 addresses. Works for both V2 pairs and V3 pools.
fn fetch_pool_tokens(pool_addr: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    use substreams_ethereum::rpc::RpcBatch;

    let result = RpcBatch::new()
        .add(
            abi::uniswap_v3_pool::functions::Token0 {},
            pool_addr.to_vec(),
        )
        .add(
            abi::uniswap_v3_pool::functions::Token1 {},
            pool_addr.to_vec(),
        )
        .execute()
        .ok()?;

    let t0 = RpcBatch::decode::<_, abi::uniswap_v3_pool::functions::Token0>(&result.responses[0])?;
    let t1 = RpcBatch::decode::<_, abi::uniswap_v3_pool::functions::Token1>(&result.responses[1])?;
    Some((t0, t1))
}

/// Scale a raw BigUint amount by dividing by 10^decimals, returning a decimal string.
fn scale_amount(amount: &BigUint, decimals: u32) -> String {
    if amount.is_zero() {
        return "0".to_string();
    }
    if decimals == 0 {
        return amount.to_string();
    }
    let divisor = BigUint::from(10u64).pow(decimals);
    let int_part = amount / &divisor;
    let frac_part = amount % &divisor;
    let frac_str = format!("{:0>width$}", frac_part, width = decimals as usize);
    let frac_trimmed = frac_str.trim_end_matches('0');
    if frac_trimmed.is_empty() {
        int_part.to_string()
    } else {
        format!("{}.{}", int_part, frac_trimmed)
    }
}

// ─── EntityChange builder helpers ─────────────────────────────────────────────

fn string_value(s: &str) -> Value {
    Value { typed: Some(Typed::String(s.to_string())) }
}

fn bigint_value(n: u64) -> Value {
    Value { typed: Some(Typed::Bigint(n.to_string())) }
}

fn bigdecimal_value(s: &str) -> Value {
    Value { typed: Some(Typed::Bigdecimal(s.to_string())) }
}

fn make_field(name: &str, value: Value) -> Field {
    Field { name: name.to_string(), new_value: value.into() }
}

// ─── Module 1: map_pool_tokens ────────────────────────────────────────────────

/// Find all V2+V3 pools that emitted Swap events this block and fetch their
/// token metadata via RPC. Feeds into store_pool_tokens (set_if_not_exists cache).
#[substreams::handlers::map]
pub fn map_pool_tokens(block: eth::Block) -> Result<PoolTokenPairs, Error> {
    let mut result = PoolTokenPairs::default();
    let mut seen: HashSet<String> = HashSet::new();

    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() {
                continue;
            }

            let topic0 = &log.topics[0];
            let protocol = if topic0 == V2_SWAP_TOPIC.as_slice() {
                "v2"
            } else if topic0 == V3_SWAP_TOPIC.as_slice() {
                "v3"
            } else {
                continue;
            };

            let pool_addr_hex = addr_hex(&log.address);
            let key = token_store_key(protocol, &pool_addr_hex);

            if seen.contains(&key) {
                continue;
            }
            seen.insert(key.clone());

            let (t0_bytes, t1_bytes) = match fetch_pool_tokens(&log.address) {
                Some(p) => p,
                None => continue,
            };

            let (t0_symbol, t0_decimals) = fetch_token_info(&t0_bytes);
            let (t1_symbol, t1_decimals) = fetch_token_info(&t1_bytes);

            result.entries.push(PoolTokenEntry {
                key,
                pair: TokenPair {
                    token0_address: addr_hex(&t0_bytes),
                    token0_symbol: t0_symbol,
                    token0_decimals: t0_decimals,
                    token1_address: addr_hex(&t1_bytes),
                    token1_symbol: t1_symbol,
                    token1_decimals: t1_decimals,
                }
                .into(),
            });
        }
    }

    Ok(result)
}

// ─── Module 2: store_pool_tokens ──────────────────────────────────────────────

/// Cache token pair metadata keyed by "<protocol>:<pool_address>".
/// set_if_not_exists → each pool fetched at most once ever.
#[substreams::handlers::store]
pub fn store_pool_tokens(pairs: PoolTokenPairs, store: StoreSetIfNotExistsProto<TokenPair>) {
    for entry in pairs.entries {
        if let Some(pair) = entry.pair.into_option() {
            store.set_if_not_exists(0, &entry.key, &pair);
        }
    }
}

// ─── Module 3a: map_v2_swaps ──────────────────────────────────────────────────

/// Extract V2 swap volumes. For each Swap event, emits |amount1| in raw units.
#[substreams::handlers::map]
pub fn map_v2_swaps(
    block: eth::Block,
    store: StoreGetProto<TokenPair>,
) -> Result<SwapVolumes, Error> {
    let mut out = SwapVolumes::default();
    let protocol = "v2";

    let timestamp = block.header.timestamp.seconds as u64;
    let day_bucket = timestamp / 86400;

    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0] != V2_SWAP_TOPIC {
                continue;
            }

            let pool_addr = addr_hex(&log.address);
            let key = token_store_key(protocol, &pool_addr);

            let pair = match store.get_last(&key) {
                Some(p) => p,
                None => continue, // metadata not yet cached; will appear next block
            };

            let swap = match abi::uniswap_v2_pair::events::Swap::match_and_decode(log) {
                Some(s) => s,
                None => continue,
            };

            // V2: |amount1| = max(amount1In, amount1Out)
            let amount1_raw = if swap.amount1_out > swap.amount1_in {
                swap.amount1_out
            } else {
                swap.amount1_in
            };

            let (_, amount1_bytes) = amount1_raw.to_bytes_be();
            let amount1_abs = BigUint::from_bytes_be(&amount1_bytes);

            out.swaps.push(SwapVolume {
                protocol: protocol.to_string(),
                pool_address: pool_addr,
                day_bucket,
                amount1_abs: amount1_abs.to_string(),
                token1_decimals: pair.token1_decimals,
                token1_symbol: pair.token1_symbol.clone(),
            });
        }
    }

    Ok(out)
}

// ─── Module 3b: map_v3_swaps ──────────────────────────────────────────────────

/// Extract V3 swap volumes. For each Swap event, emits |amount1| (abs of int256).
#[substreams::handlers::map]
pub fn map_v3_swaps(
    block: eth::Block,
    store: StoreGetProto<TokenPair>,
) -> Result<SwapVolumes, Error> {
    let mut out = SwapVolumes::default();
    let protocol = "v3";

    let timestamp = block.header.timestamp.seconds as u64;
    let day_bucket = timestamp / 86400;

    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0] != V3_SWAP_TOPIC {
                continue;
            }

            let pool_addr = addr_hex(&log.address);
            let key = token_store_key(protocol, &pool_addr);

            let pair = match store.get_last(&key) {
                Some(p) => p,
                None => continue,
            };

            let swap = match abi::uniswap_v3_pool::events::Swap::match_and_decode(log) {
                Some(s) => s,
                None => continue,
            };

            // V3: amount1 is int256 (signed); take absolute value
            let amount1_signed = swap.amount1.to_signed_bytes_be();
            let amount1_bi = num_bigint::BigInt::from_signed_bytes_be(&amount1_signed);
            let amount1_abs: BigUint = amount1_bi.magnitude().clone();

            out.swaps.push(SwapVolume {
                protocol: protocol.to_string(),
                pool_address: pool_addr,
                day_bucket,
                amount1_abs: amount1_abs.to_string(),
                token1_decimals: pair.token1_decimals,
                token1_symbol: pair.token1_symbol.clone(),
            });
        }
    }

    Ok(out)
}

// ─── Module 4: store_daily_volume ─────────────────────────────────────────────

/// Additive store: accumulates volume per (protocol, pool, day).
/// Key: "<protocol>:<pool_address>:<day_bucket>"
/// value = sum of (|amount1| / 10^decimals1) across all swaps in this pool-day.
#[substreams::handlers::store]
pub fn store_daily_volume(
    v2_swaps: SwapVolumes,
    v3_swaps: SwapVolumes,
    store: StoreAddBigDecimal,
) {
    for swap in v2_swaps.swaps.iter().chain(v3_swaps.swaps.iter()) {
        let amount1_abs = match swap.amount1_abs.parse::<BigUint>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        if amount1_abs.is_zero() {
            continue;
        }

        let volume_str = scale_amount(&amount1_abs, swap.token1_decimals);
        let volume = match BigDecimal::try_from(volume_str.as_str()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let key = volume_key(&swap.protocol, &swap.pool_address, swap.day_bucket);
        store.add(0, key, volume);
    }
}

// ─── Module 5: graph_out ──────────────────────────────────────────────────────

/// Derives EntityChanges from store_daily_volume deltas.
/// First write to a (protocol, pool, day) key → CREATE entity.
/// Subsequent adds → UPDATE entity with new cumulative volume.
#[substreams::handlers::map]
pub fn graph_out(
    deltas: Deltas<DeltaBigDecimal>,
) -> Result<EntityChanges, Error> {
    let mut entity_changes: Vec<EntityChange> = Vec::new();

    for delta in &deltas.deltas {
        let key = &delta.key;
        // Key format: "<protocol>:<pool_address>:<day_bucket>"
        // pool_address contains "0x..." (no colons), so splitn(3, ':') is safe.
        let parts: Vec<&str> = key.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }

        let protocol = parts[0];
        let pool = parts[1];
        let day_str = parts[2];

        let day_bucket: u64 = match day_str.parse() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let new_volume = delta.new_value.to_string();
        let is_create = delta.operation == DeltaOperation::Create;

        if is_create {
            entity_changes.push(EntityChange {
                entity: "PoolDayVolume".to_string(),
                id: key.clone(),
                ordinal: 0,
                operation: Operation::Create.into(),
                fields: vec![
                    make_field("id", string_value(key)),
                    make_field("pool", string_value(pool)),
                    make_field("protocol", string_value(protocol)),
                    make_field("dayBucket", bigint_value(day_bucket)),
                    make_field("volumeToken1", bigdecimal_value(&new_volume)),
                ],
            });
        } else {
            entity_changes.push(EntityChange {
                entity: "PoolDayVolume".to_string(),
                id: key.clone(),
                ordinal: 0,
                operation: Operation::Update.into(),
                fields: vec![
                    make_field("volumeToken1", bigdecimal_value(&new_volume)),
                ],
            });
        }
    }

    Ok(EntityChanges { entity_changes })
}
