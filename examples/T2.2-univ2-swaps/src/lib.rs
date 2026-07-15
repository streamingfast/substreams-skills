mod abi;
mod pb {
    pub mod uniswap {
        pub mod v2 {
            include!(concat!(env!("OUT_DIR"), "/uniswap.v2.rs"));
        }
    }
}

use std::str::FromStr;

use substreams::errors::Error;
use substreams::scalar::BigInt;
use substreams::store::{
    StoreGet, StoreGetString, StoreNew, StoreSetIfNotExists, StoreSetIfNotExistsString,
};
use substreams::Hex;
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::rpc::RpcBatch;
use substreams_ethereum::Event;

use pb::uniswap::v2::{PoolEvents, PoolSwapEvent, Swap, Swaps};

// ── Swap topic0 ────────────────────────────────────────────────────────────────
// keccak256("Swap(address,uint256,uint256,uint256,uint256,address)")
// = 0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822
const SWAP_TOPIC: [u8; 32] = [
    0xd7, 0x8a, 0xd9, 0x5f, 0xa4, 0x6c, 0x99, 0x4b, 0x65, 0x51, 0xd0, 0xda, 0x85, 0xfc, 0x27,
    0x5f, 0xe6, 0x13, 0xce, 0x37, 0x65, 0x7f, 0xb8, 0xd5, 0xe3, 0xd1, 0x30, 0x84, 0x01, 0x59,
    0xd8, 0x22,
];

// ── Module 1: map_pool_events ──────────────────────────────────────────────────
// Scans the block for UniswapV2 Swap events (by topic0) and emits raw
// (unscaled) PoolSwapEvent records.
#[substreams::handlers::map]
pub fn map_pool_events(block: eth::Block) -> Result<PoolEvents, Error> {
    let mut events: Vec<PoolSwapEvent> = vec![];
    let block_number = block.number;

    for trx in block.transactions() {
        let tx_hash = Hex::encode(&trx.hash);

        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0].as_slice() != SWAP_TOPIC {
                continue;
            }

            let swap = match abi::uniswap_v2_pair::events::Swap::match_and_decode(log) {
                Some(e) => e,
                None => continue,
            };

            events.push(PoolSwapEvent {
                pool_address: Hex::encode(&log.address),
                amount0_in: swap.amount0_in.to_string(),
                amount1_in: swap.amount1_in.to_string(),
                amount0_out: swap.amount0_out.to_string(),
                amount1_out: swap.amount1_out.to_string(),
                sender: Hex::encode(&swap.sender),
                recipient: Hex::encode(&swap.to),
                transaction_hash: tx_hash.clone(),
                log_index: log.index,
                block_number,
            });
        }
    }

    Ok(PoolEvents { events })
}

// ── Module 2: store_pool_tokens ────────────────────────────────────────────────
// For every pool address seen in PoolEvents, fetch token0/token1 metadata via
// batched eth_call (RpcBatch) and cache as JSON under "pool:{address}".
//
// updatePolicy: set_if_not_exists means the store runtime skips the write if
// the key already exists — so each pool's metadata is fetched exactly once
// across all blocks, never again.
//
// Prefer the T3.1 layout (map fetch → store cache → map consume) for new code;
// this example keeps RPC in the store handler for a smaller module graph.
#[substreams::handlers::store]
pub fn store_pool_tokens(events: PoolEvents, store: StoreSetIfNotExistsString) {
    for event in &events.events {
        let key = format!("pool:{}", event.pool_address);

        let pool_bytes = match hex::decode(&event.pool_address) {
            Ok(b) => b,
            Err(_) => continue,
        };

        // Batch token0() + token1() on the pair (never one execute() per field)
        let pair_rpc = RpcBatch::new()
            .add(
                abi::uniswap_v2_pair::functions::Token0 {},
                pool_bytes.clone(),
            )
            .add(abi::uniswap_v2_pair::functions::Token1 {}, pool_bytes)
            .execute();

        let (token0_addr, token1_addr) = match pair_rpc {
            Ok(resp) => {
                let t0 = RpcBatch::decode::<_, abi::uniswap_v2_pair::functions::Token0>(
                    &resp.responses[0],
                );
                let t1 = RpcBatch::decode::<_, abi::uniswap_v2_pair::functions::Token1>(
                    &resp.responses[1],
                );
                match (t0, t1) {
                    (Some(a), Some(b)) => (a, b),
                    _ => continue,
                }
            }
            Err(_) => continue,
        };

        // Batch symbol + decimals for both tokens (4 calls, one round-trip)
        let meta_rpc = RpcBatch::new()
            .add(abi::erc20::functions::Symbol {}, token0_addr.clone())
            .add(abi::erc20::functions::Decimals {}, token0_addr.clone())
            .add(abi::erc20::functions::Symbol {}, token1_addr.clone())
            .add(abi::erc20::functions::Decimals {}, token1_addr.clone())
            .execute();

        let (sym0, dec0, sym1, dec1) = match meta_rpc {
            Ok(resp) => {
                let s0 = symbol_from_response(&resp.responses[0], &token0_addr);
                let d0 = decimals_from_response(&resp.responses[1]);
                let s1 = symbol_from_response(&resp.responses[2], &token1_addr);
                let d1 = decimals_from_response(&resp.responses[3]);
                (s0, d0, s1, d1)
            }
            Err(_) => continue,
        };

        let t0_hex = Hex::encode(&token0_addr);
        let t1_hex = Hex::encode(&token1_addr);

        // Serialize as compact JSON into the string-typed store.
        // Format: {"t0":"<addr>","s0":"<sym>","d0":<dec>,"t1":"<addr>","s1":"<sym>","d1":<dec>}
        let value = format!(
            "{{\"t0\":\"{}\",\"s0\":\"{}\",\"d0\":{},\"t1\":\"{}\",\"s1\":\"{}\",\"d1\":{}}}",
            t0_hex, sym0, dec0, t1_hex, sym1, dec1,
        );

        store.set_if_not_exists(0, key, &value);
    }
}

fn normalize_symbol(s: String) -> String {
    s.trim_end_matches('\0').trim().to_string()
}

/// Decode ERC-20 symbol from a batch response. On decode failure (e.g. bytes32
/// symbol like MKR) or empty string, fall back to the 0x-prefixed token address.
fn symbol_from_response(
    response: &substreams_ethereum::pb::eth::rpc::RpcResponse,
    token_addr: &[u8],
) -> String {
    match RpcBatch::decode::<_, abi::erc20::functions::Symbol>(response) {
        Some(s) => {
            let trimmed = normalize_symbol(s);
            if trimmed.is_empty() {
                format!("0x{}", Hex::encode(token_addr))
            } else {
                trimmed
            }
        }
        None => format!("0x{}", Hex::encode(token_addr)),
    }
}

fn decimals_from_response(response: &substreams_ethereum::pb::eth::rpc::RpcResponse) -> u32 {
    RpcBatch::decode::<_, abi::erc20::functions::Decimals>(response)
        .map(|d| d.to_string().parse::<u32>().unwrap_or(18))
        .unwrap_or(18)
}

// ── Module 3: map_swaps ────────────────────────────────────────────────────────
// Joins raw PoolEvents with the pool-token store to produce fully annotated,
// human-readable Swap records.
#[substreams::handlers::map]
pub fn map_swaps(events: PoolEvents, store: StoreGetString) -> Result<Swaps, Error> {
    let mut swaps: Vec<Swap> = vec![];

    for event in events.events {
        let key = format!("pool:{}", event.pool_address);

        let meta_json = match store.get_last(&key) {
            Some(v) => v,
            None => {
                // Store miss: first block this pool appears — metadata not yet stored.
                // This can happen because store_pool_tokens and map_swaps run in the
                // same block for new pools.  The store deltas are not yet readable via
                // get_last at the start of this block.  The swap will be skipped; the
                // pool will be cached and available from the *next* block onward.
                continue;
            }
        };

        let (t0_hex, sym0, dec0, t1_hex, sym1, dec1) = match parse_pool_meta(&meta_json) {
            Some(m) => m,
            None => continue,
        };

        let amount0_in = format_amount(&event.amount0_in, dec0);
        let amount1_in = format_amount(&event.amount1_in, dec1);
        let amount0_out = format_amount(&event.amount0_out, dec0);
        let amount1_out = format_amount(&event.amount1_out, dec1);

        swaps.push(Swap {
            pool_address: event.pool_address,
            token0_address: t0_hex,
            token0_symbol: sym0,
            token0_decimals: dec0,
            token1_address: t1_hex,
            token1_symbol: sym1,
            token1_decimals: dec1,
            amount0_in,
            amount1_in,
            amount0_out,
            amount1_out,
            sender: event.sender,
            recipient: event.recipient,
            transaction_hash: event.transaction_hash,
            log_index: event.log_index,
            block_number: event.block_number,
        });
    }

    Ok(Swaps { swaps })
}

/// Minimal JSON parser for the fixed-format string we write in store_pool_tokens:
/// {"t0":"<addr>","s0":"<sym>","d0":<dec>,"t1":"<addr>","s1":"<sym>","d1":<dec>}
fn parse_pool_meta(json: &str) -> Option<(String, String, u32, String, String, u32)> {
    fn extract_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
        let pattern = format!("\"{}\":\"", key);
        let start = json.find(pattern.as_str())? + pattern.len();
        let end = json[start..].find('"')? + start;
        Some(&json[start..end])
    }
    fn extract_num(json: &str, key: &str) -> Option<u32> {
        let pattern = format!("\"{}\":", key);
        let start = json.find(pattern.as_str())? + pattern.len();
        let end_rel = json[start..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(json[start..].len());
        json[start..start + end_rel].parse().ok()
    }

    let t0 = extract_str(json, "t0")?.to_string();
    let s0 = extract_str(json, "s0")?.to_string();
    let d0 = extract_num(json, "d0")?;
    let t1 = extract_str(json, "t1")?.to_string();
    let s1 = extract_str(json, "s1")?.to_string();
    let d1 = extract_num(json, "d1")?;

    Some((t0, s0, d0, t1, s1, d1))
}

/// Scale a raw uint256 decimal string by `decimals` to produce a human-readable
/// decimal number.  E.g. "1000000000000000000" with decimals=18 → "1".
fn format_amount(raw: &str, decimals: u32) -> String {
    let big = match BigInt::from_str(raw) {
        Ok(v) => v,
        Err(_) => return "0".to_string(),
    };
    let s = big.to_string();

    if s == "0" || s.is_empty() {
        return "0".to_string();
    }

    let (sign, digits): (&str, &str) = if s.starts_with('-') {
        ("-", &s[1..])
    } else {
        ("", s.as_str())
    };

    let dec = decimals as usize;
    if dec == 0 {
        return format!("{}{}", sign, digits);
    }

    let result: String = if digits.len() <= dec {
        // Need leading zeros after decimal point: e.g. "0.000123"
        let padded = format!("{:0>width$}", digits, width = dec + 1);
        let (int_part, frac_part) = padded.split_at(padded.len() - dec);
        let frac_trimmed = frac_part.trim_end_matches('0');
        if frac_trimmed.is_empty() {
            int_part.to_string()
        } else {
            format!("{}.{}", int_part, frac_trimmed)
        }
    } else {
        let split = digits.len() - dec;
        let int_part = &digits[..split];
        let frac_part = &digits[split..];
        let frac_trimmed = frac_part.trim_end_matches('0');
        if frac_trimmed.is_empty() {
            int_part.to_string()
        } else {
            format!("{}.{}", int_part, frac_trimmed)
        }
    };

    format!("{}{}", sign, result)
}
