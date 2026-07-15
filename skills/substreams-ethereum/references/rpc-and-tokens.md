# eth_call, RpcBatch & Token Metadata Stores

Deep dive for the `substreams-ethereum` skill. Covers reading contract state that logs do not carry.

**This is the highest-value pattern in the skill.** Eval T3.1: agents that skipped pool token resolution averaged **41%** correctness; with the batch + cache pattern below, **100%**.

## When you need RPC at all

Ask first: **is the field already in the log?** Uniswap V2's `Swap` carries the amounts; it does not carry `token0`/`token1` (those are pool state) or token `decimals` (those live on the token contract). Only reach for RPC when the event genuinely lacks the data.

| Need | Source |
|---|---|
| Event params (amounts, sender, recipient) | The log — no RPC |
| Pool's `token0` / `token1` | RPC, once per pool, cached |
| Token `symbol` / `decimals` / `name` | RPC, once per token, cached |
| Balances, reserves at a block | RPC — expensive, prefer deriving from events |

RPC calls cost real time, are not available on every network or tier, and are the usual reason a Substreams is slow.

## RpcBatch basics

`Abigen` generates a callable struct per contract function under `abi::<name>::functions::`.

```rust
use substreams_ethereum::rpc::RpcBatch;

let batch = RpcBatch::new()
    .add(abi::erc20::functions::Symbol {}, token_addr.to_vec())
    .add(abi::erc20::functions::Decimals {}, token_addr.to_vec())
    .execute();

let (symbol, decimals) = match batch {
    Ok(resp) => (
        RpcBatch::decode::<_, abi::erc20::functions::Symbol>(&resp.responses[0])
            .unwrap_or_else(|| "UNKNOWN".to_string()),
        RpcBatch::decode::<_, abi::erc20::functions::Decimals>(&resp.responses[1])
            .map(|d| d.to_u64() as u32)
            .unwrap_or(18u32),
    ),
    Err(_) => ("UNKNOWN".to_string(), 18u32),
};
```

Rules:

1. **`responses[i]` corresponds to `.add(…)` call order.** Decode with the matching function type or you get `None`.
2. **`decode` returns `Option`** — it is `None` for a reverted call or a non-compliant return. Never `unwrap()`.
3. **Functions with args** take them in the struct: `.add(abi::pool::functions::GetAmount { amount_in: x.clone() }, addr.to_vec())`.
4. **Batch aggressively.** One `execute()` with 8 calls beats 8 `execute()`s by roughly the round-trip count.

## The cache-store pattern (three modules)

Never call RPC for the same contract twice. Split into `map` (fetch) → `store` (cache) → `map` (consume).

### Module 1 — fetch, deduplicated within the block

```rust
#[substreams::handlers::map]
pub fn map_pool_tokens(block: eth::Block) -> Result<PoolTokenPairs, Error> {
    let mut result = PoolTokenPairs::default();
    let mut seen: HashSet<Vec<u8>> = HashSet::new();   // in-block dedup only

    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
                continue;
            }
            let pool = log.address.clone();
            if !seen.insert(pool.clone()) {
                continue;                               // already fetched this block
            }

            let rpc = RpcBatch::new()
                .add(abi::pool::functions::Token0 {}, pool.clone())
                .add(abi::pool::functions::Token1 {}, pool.clone())
                .execute();

            let (t0, t1) = match rpc {
                Ok(resp) => match (
                    RpcBatch::decode::<_, abi::pool::functions::Token0>(&resp.responses[0]),
                    RpcBatch::decode::<_, abi::pool::functions::Token1>(&resp.responses[1]),
                ) {
                    (Some(a), Some(b)) => (a, b),
                    _ => continue,                      // not a pool / reverted
                },
                Err(_) => continue,
            };

            // Emit 0x-prefixed addresses; use the same format as store keys and get_last lookups.
            result.entries.push(PoolTokenEntry {
                pool: format!("0x{}", Hex::encode(&pool)),
                /* token0/token1 similarly with 0x prefix */
            });
        }
    }
    Ok(result)
}
```

The `HashSet` here is **correct** — it dedups within a single block. It is *not* a cross-block cache; that is the store's job.

### Module 2 — cache

```yaml
- name: store_pool_tokens
  kind: store
  updatePolicy: set_if_not_exists   # write once per pool, ever
  valueType: proto:uniswap.v3.swaps.TokenPair
  inputs:
    - map: map_pool_tokens
```

```rust
// These imports are load-bearing — the handler does not compile without them.
// StoreNew: the #[handlers::store] macro expands to a `::new()` call.
// StoreSetIfNotExists: provides `.set_if_not_exists()` (a trait method, not inherent).
// Note the *Proto type lives in `prelude`, while the traits live in `store`.
use substreams::prelude::StoreSetIfNotExistsProto;
use substreams::store::{StoreNew, StoreSetIfNotExists};

#[substreams::handlers::store]
pub fn store_pool_tokens(pairs: PoolTokenPairs, store: StoreSetIfNotExistsProto<TokenPair>) {
    for entry in pairs.entries {
        // ord is u64; value is passed by reference
        store.set_if_not_exists(0, &entry.pool, entry.tokens.as_ref().unwrap());
    }
}
```

`set_if_not_exists` is the point: the first block that sees a pool pays the RPC, every later block reads the store for free.

### Module 3 — consume

```yaml
- name: map_swaps
  kind: map
  inputs:
    - source: sf.ethereum.type.v2.Block
    - store: store_pool_tokens
      mode: get
```

```rust
use substreams::store::{StoreGet, StoreGetProto};   // StoreGet provides `.get_last()`

#[substreams::handlers::map]
pub fn map_swaps(block: eth::Block, store: StoreGetProto<TokenPair>) -> Result<SwapEvents, Error> {
    let mut swaps = Vec::new();

    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            if log.topics.is_empty() || log.topics[0] != SWAP_TOPIC {
                continue;
            }

            // Key must match what map_pool_tokens / store_pool_tokens wrote (same 0x policy).
            let key = format!("0x{}", Hex::encode(&log.address));
            let tokens = match store.get_last(&key) {
                Some(t) => t,
                None => continue,          // pool not resolved yet — skip
            };

            swaps.push(decode_swap(log, &tokens));
        }
    }

    Ok(SwapEvents { swaps })
}
```

### Why not a `HashMap` in the map handler

A `HashMap` inside a map handler is **rebuilt from empty on every block** — the WASM module is a pure function of its inputs and keeps no state between blocks. Every block re-issues every RPC call. This looks like a cache and is the opposite of one. Cross-block memoization must be a **store**.

## Failure handling

Non-compliant tokens are common on mainnet. Plan for them:

| Case | Behavior | Handling |
|---|---|---|
| `symbol()` returns `bytes32` (e.g. MKR, older tokens) | `decode::<String>` → `None` | Fall back to a `bytes32` ABI variant or a constant |
| Contract has no `decimals()` | `None` | Default to 18 **and record that you did** |
| Call reverts at that block (contract not yet deployed) | `None` | Skip; a later block will populate the store |
| Proxy contract | May succeed | Calls hit the proxy, which delegates — usually fine |

Defaulting `decimals` to 18 for a 6-decimal token misprices by **10¹²**. If a default is load-bearing for user-facing numbers, surface it as a field (`decimals_resolved: bool`) rather than burying it.

## Amount math

Scaling a raw amount by a token's decimals is already in the crate — **do not hand-roll it**:

```rust
use substreams::scalar::BigInt;

// BigInt::to_decimal(decimals: u64) -> BigDecimal  — i.e. raw / 10^decimals
let human = raw_amount.to_decimal(decimals as u64);
```

* `uint256` **never** fits in `u64` (max ~1.8e19; a single 18-decimal token can exceed it). Use `BigInt`.
* Emit as a decimal `string` in protobuf — proto3 has no 256-bit type, and `double` loses precision.
* `substreams::scalar::BigInt` wraps `num_bigint::BigInt` — it is **arbitrary precision**, not 256-bit. Squaring `sqrtPriceX96` will not overflow it. Reach for `BigDecimal` when you need fractional results, not to dodge an overflow that cannot happen.
* If you drop to `num_bigint` directly (T3.1 does — it squares `sqrtPriceX96` as a `num_bigint::BigUint`), add `num-bigint = "0.4"` and `num-traits = "0.2"` to `Cargo.toml`; they are **not** transitively available through `substreams`. Staying on `substreams::scalar::BigInt` needs no extra dependency and is the simpler default.
* Uniswap V3 price: `(sqrtPriceX96 / 2^96)^2 * 10^(decimals0 - decimals1)`. Watch the **sign** of V3's `int256` amounts and the `decimals0 - decimals1` exponent, which can be negative.

## Cost checklist

1. Is the field already in the log? → no RPC.
2. Batched into one `RpcBatch`? → not one `execute()` per field.
3. Cached in a `set_if_not_exists` store, keyed by address? → not a `HashMap`.
4. Deduplicated within the block before calling?
5. Every `decode` handled as `Option`, no `unwrap()`?
6. Amounts in `BigInt`/`BigDecimal`, emitted as strings?

If a run is slow, check 2 and 3 first — they account for most EVM Substreams performance problems.
