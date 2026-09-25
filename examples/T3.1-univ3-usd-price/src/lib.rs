mod abi;
mod pb;

use std::collections::HashSet;

use num_bigint::{BigInt, BigUint, Sign};
use num_traits::Zero;
use substreams::errors::Error;
use substreams::prelude::StoreSetIfNotExistsProto;
use substreams::store::{StoreGet, StoreGetProto, StoreNew, StoreSetIfNotExists};
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::Event;

use pb::uniswap::v3::swaps::{
    PoolTokenEntry, PoolTokenPairs, SwapEvent, SwapEvents, TokenPair,
};

// Uniswap V3 Swap event topic0: keccak256("Swap(address,address,int256,int256,uint160,uint128,int24)")
const SWAP_TOPIC: [u8; 32] =
    hex_literal::hex!("c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67");

// USD stablecoin addresses (checksummed — comparison is done case-insensitively via bytes)
const USDC: [u8; 20] = hex_literal::hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
const USDT: [u8; 20] = hex_literal::hex!("dAC17F958D2ee523a2206206994597C13D831ec7");
const DAI: [u8; 20] = hex_literal::hex!("6B175474E89094C44Da98b954EedeAC495271d0F");

fn is_stable(addr: &[u8]) -> bool {
    addr == USDC || addr == USDT || addr == DAI
}

fn addr_to_hex(addr: &[u8]) -> String {
    format!("0x{}", hex::encode(addr))
}

/// Fetch token metadata (symbol, decimals) via RPC batch for a given ERC-20 address.
fn get_token_info(addr: &[u8]) -> (String, u32) {
    use substreams_ethereum::rpc::RpcBatch;

    let result = RpcBatch::new()
        .add(abi::erc20::functions::Symbol {}, addr.to_vec())
        .add(abi::erc20::functions::Decimals {}, addr.to_vec())
        .execute();

    if let Ok(resp) = result {
        let symbol =
            RpcBatch::decode::<_, abi::erc20::functions::Symbol>(&resp.responses[0])
                .unwrap_or_else(|| "UNKNOWN".to_string());
        let decimals =
            RpcBatch::decode::<_, abi::erc20::functions::Decimals>(&resp.responses[1])
                .map(|d| d.to_u64() as u32)
                .unwrap_or(18u32);
        return (symbol, decimals);
    }

    // Fallback hardcodes for known stablecoins in case RPC call fails
    if addr == USDC {
        return ("USDC".to_string(), 6);
    }
    if addr == USDT {
        return ("USDT".to_string(), 6);
    }
    if addr == DAI {
        return ("DAI".to_string(), 18);
    }

    ("UNKNOWN".to_string(), 18)
}

/// Module 1: For each block, find pools that emitted Swap events and fetch their token metadata.
/// This feeds into the store which caches token pairs keyed by pool address.
#[substreams::handlers::map]
pub fn map_pool_tokens(block: eth::Block) -> Result<PoolTokenPairs, Error> {
    use substreams_ethereum::rpc::RpcBatch;

    let mut result = PoolTokenPairs::default();

    // Deduplicate: one RPC call per unique pool address per block
    let mut seen: HashSet<Vec<u8>> = HashSet::new();

    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
                continue;
            }

            let pool_addr = log.address.clone();
            if seen.contains(&pool_addr) {
                continue;
            }
            seen.insert(pool_addr.clone());

            // Fetch token0 and token1 addresses from pool contract
            let pool_rpc = RpcBatch::new()
                .add(abi::uniswap_v3_pool::functions::Token0 {}, pool_addr.clone())
                .add(abi::uniswap_v3_pool::functions::Token1 {}, pool_addr.clone())
                .execute();

            let (token0_addr, token1_addr) = match pool_rpc {
                Ok(resp) => {
                    let t0 = RpcBatch::decode::<_, abi::uniswap_v3_pool::functions::Token0>(
                        &resp.responses[0],
                    );
                    let t1 = RpcBatch::decode::<_, abi::uniswap_v3_pool::functions::Token1>(
                        &resp.responses[1],
                    );
                    match (t0, t1) {
                        (Some(a), Some(b)) => (a, b),
                        _ => continue,
                    }
                }
                Err(_) => continue,
            };

            // Fetch symbol + decimals for both tokens
            let (token0_symbol, token0_decimals) = get_token_info(&token0_addr);
            let (token1_symbol, token1_decimals) = get_token_info(&token1_addr);

            result.entries.push(PoolTokenEntry {
                pool_address: addr_to_hex(&pool_addr),
                pair: TokenPair {
                    token0_address: addr_to_hex(&token0_addr),
                    token0_symbol,
                    token0_decimals,
                    token1_address: addr_to_hex(&token1_addr),
                    token1_symbol,
                    token1_decimals,
                }
                .into(),
            });
        }
    }

    Ok(result)
}

/// Module 2 (store): Caches token pair metadata keyed by pool address.
/// Uses set_if_not_exists so each pool is fetched at most once across all blocks.
#[substreams::handlers::store]
pub fn store_pool_tokens(pairs: PoolTokenPairs, store: StoreSetIfNotExistsProto<TokenPair>) {
    for entry in pairs.entries {
        if let Some(pair) = entry.pair.into_option() {
            store.set_if_not_exists(0, &entry.pool_address, &pair);
        }
    }
}

/// Format a BigInt as human-readable decimal with `decimals` fractional digits.
/// Preserves sign: negative values get a leading '-'.
fn format_decimal(value: &BigInt, decimals: u32) -> String {
    if value.sign() == Sign::NoSign {
        return "0".to_string();
    }

    let is_neg = value.sign() == Sign::Minus;
    let abs_val = value.magnitude().clone();

    if decimals == 0 {
        return format!("{}{}", if is_neg { "-" } else { "" }, abs_val);
    }

    let divisor = BigUint::from(10u64).pow(decimals);
    let int_part = &abs_val / &divisor;
    let frac_part = &abs_val % &divisor;

    let sign = if is_neg { "-" } else { "" };
    let frac_str = format!("{:0>width$}", frac_part, width = decimals as usize);
    let frac_trimmed = frac_str.trim_end_matches('0');

    if frac_trimmed.is_empty() {
        format!("{}{}", sign, int_part)
    } else {
        format!("{}{}.{}", sign, int_part, frac_trimmed)
    }
}

/// Compute price of token0 in token1 from sqrtPriceX96.
///
/// Formula: price = (sqrtPriceX96 / 2^96)^2 * 10^(decimals0 - decimals1)
///
/// We compute with 18 digits of fractional precision using pure integer arithmetic.
fn compute_price(sqrt_price_x96: &BigUint, decimals0: u32, decimals1: u32) -> String {
    if sqrt_price_x96.is_zero() {
        return "0".to_string();
    }

    // sq = sqrtPriceX96^2
    let sq = sqrt_price_x96 * sqrt_price_x96;
    // two_pow_192 = 2^192  (since (2^96)^2 = 2^192)
    let two_pow_192 = BigUint::from(2u64).pow(192u32);

    // We want: result = sq / 2^192 * 10^decimals0 / 10^decimals1
    // With 18 decimal places of output precision:
    //   scaled = sq * 10^(18 + decimals0) / (2^192 * 10^decimals1)
    const PRECISION: u32 = 18;

    let scale_up = BigUint::from(10u64).pow(PRECISION + decimals0);
    let numerator = sq * scale_up;

    let scale_down = BigUint::from(10u64).pow(decimals1);
    let denominator = two_pow_192 * scale_down;

    let scaled = numerator / denominator;

    let divisor = BigUint::from(10u64).pow(PRECISION);
    let int_part = &scaled / &divisor;
    let frac_part = &scaled % &divisor;

    let frac_str = format!("{:0>width$}", frac_part, width = PRECISION as usize);
    let frac_trimmed = frac_str.trim_end_matches('0');

    if frac_trimmed.is_empty() {
        format!("{}", int_part)
    } else {
        format!("{}.{}", int_part, frac_trimmed)
    }
}

/// Derive USD price of token0.
/// - token1 is a stable → price_token0_in_token1 = USD price
/// - token0 is a stable → USD price is 1.0
/// - neither → empty string (null)
fn compute_usd_price(
    sqrt_price_x96: &BigUint,
    token0_addr_hex: &str,
    token1_addr_hex: &str,
    decimals0: u32,
    decimals1: u32,
) -> String {
    // Compare by lowercasing the hex strings (addresses are case-insensitive)
    let t0 = token0_addr_hex.to_lowercase();
    let t1 = token1_addr_hex.to_lowercase();

    let usdc_hex = format!("0x{}", hex::encode(USDC)).to_lowercase();
    let usdt_hex = format!("0x{}", hex::encode(USDT)).to_lowercase();
    let dai_hex = format!("0x{}", hex::encode(DAI)).to_lowercase();

    if t1 == usdc_hex || t1 == usdt_hex || t1 == dai_hex {
        // token1 is a stable: price of token0 in token1 = USD price of token0
        return compute_price(sqrt_price_x96, decimals0, decimals1);
    }

    if t0 == usdc_hex || t0 == usdt_hex || t0 == dai_hex {
        // token0 is a stable: its USD price = 1.0
        return "1.0".to_string();
    }

    // Neither is a stable — cannot derive USD price without cross-pool routing
    String::new()
}

/// Module 3: For each Swap event, look up cached token metadata from the store,
/// compute prices, and emit a SwapEvent.
#[substreams::handlers::map]
pub fn map_swaps(
    block: eth::Block,
    store: StoreGetProto<TokenPair>,
) -> Result<SwapEvents, Error> {
    let mut swaps = SwapEvents::default();

    for trx in block.transactions() {
        let tx_hash = hex::encode(&trx.hash);

        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
                continue;
            }

            // Decode the Swap event using ABI-generated bindings
            let swap = match abi::uniswap_v3_pool::events::Swap::match_and_decode(log) {
                Some(s) => s,
                None => continue,
            };

            let pool_addr_hex = addr_to_hex(&log.address);

            // Look up token metadata from store
            let pair = match store.get_last(&pool_addr_hex) {
                Some(p) => p,
                None => {
                    // Store miss — pool not seen in a prior block; skip
                    // (will be picked up in next block's map_pool_tokens pass)
                    continue;
                }
            };

            let token0_addr = &pair.token0_address;
            let token1_addr = &pair.token1_address;
            let decimals0 = pair.token0_decimals;
            let decimals1 = pair.token1_decimals;

            // Decode signed int256 amounts using big-endian signed bytes
            let amount0_bi = BigInt::from_signed_bytes_be(&swap.amount0.to_signed_bytes_be());
            let amount1_bi = BigInt::from_signed_bytes_be(&swap.amount1.to_signed_bytes_be());

            let amount0_str = format_decimal(&amount0_bi, decimals0);
            let amount1_str = format_decimal(&amount1_bi, decimals1);

            // sqrtPriceX96 is uint160 — use unsigned conversion
            let (_, sqrt_bytes) = swap.sqrt_price_x96.to_bytes_be();
            let sqrt_price = BigUint::from_bytes_be(&sqrt_bytes);

            let price_t0_in_t1 = compute_price(&sqrt_price, decimals0, decimals1);
            let usd_price = compute_usd_price(
                &sqrt_price,
                token0_addr,
                token1_addr,
                decimals0,
                decimals1,
            );

            swaps.swaps.push(SwapEvent {
                pool_address: pool_addr_hex,
                token0_address: token0_addr.clone(),
                token0_symbol: pair.token0_symbol.clone(),
                token0_decimals: decimals0,
                token1_address: token1_addr.clone(),
                token1_symbol: pair.token1_symbol.clone(),
                token1_decimals: decimals1,
                amount0: amount0_str,
                amount1: amount1_str,
                price_token0_in_token1: price_t0_in_t1,
                usd_price_token0: usd_price,
                tx_hash: tx_hash.clone(),
                log_index: log.index,
                block_number: block.number,
            });
        }
    }

    Ok(swaps)
}
