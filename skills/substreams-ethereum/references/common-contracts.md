# Common Contracts, Topic0 Hashes & Standards

Reference values for the `substreams-ethereum` skill. Offer these as **choices** during pre-flight — never assume the user wants a specific contract.

> Every topic0 below was computed as `keccak256(<canonical signature>)` and cross-checked against the working example projects in this repo. The canonical signature has **no parameter names and no spaces**, and uses full type names (`uint256`, not `uint`).

## Standard token events

| Event | Canonical signature | topic0 |
|---|---|---|
| ERC-20 / ERC-721 Transfer | `Transfer(address,address,uint256)` | `ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef` |
| ERC-20 / ERC-721 Approval | `Approval(address,address,uint256)` | `8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925` |
| ERC-1155 TransferSingle | `TransferSingle(address,address,address,uint256,uint256)` | `c3d58168c5ae7397731d063d5bbf3d657854427343f4c083240f7aacaa2d0f62` |
| ERC-1155 TransferBatch | `TransferBatch(address,address,address,uint256[],uint256[])` | `4a39dc06d4c0dbc64b70af90fd698a233a518aa5d07e595d983b8c0526c8f7fb` |
| WETH Deposit | `Deposit(address,uint256)` | `e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c` |
| WETH Withdrawal | `Withdrawal(address,uint256)` | `7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65` |

### ERC-20 vs ERC-721 Transfer share a topic0

Both are `Transfer(address,address,uint256)` — **the same hash**. They differ by indexing:

| | Indexed params | `topics.len()` | `data` |
|---|---|---|---|
| **ERC-20** | `from`, `to` | 3 | 32 bytes = `value` |
| **ERC-721** | `from`, `to`, `tokenId` | 4 | empty |

So `topics.len() == 4` ⇒ NFT transfer; `topics.len() == 3` ⇒ fungible transfer. Filtering by contract address is the reliable disambiguator; `topics.len()` is the fallback when scanning chain-wide.

**Mint detection** (T2.1): `from` is the zero address, i.e. `topics[1]` is 32 zero bytes. **Burn**: `to` is the zero address.

```rust
const ZERO_TOPIC: [u8; 32] = [0u8; 32];
let is_mint = log.topics[1] == ZERO_TOPIC;
```

## Uniswap

| Event | Canonical signature | topic0 |
|---|---|---|
| V2 Swap | `Swap(address,uint256,uint256,uint256,uint256,address)` | `d78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822` |
| V2 Sync | `Sync(uint112,uint112)` | `1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1` |
| V2 PairCreated | `PairCreated(address,address,address,uint256)` | `0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9` |
| V3 Swap | `Swap(address,address,int256,int256,uint160,uint128,int24)` | `c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67` |
| V3 PoolCreated | `PoolCreated(address,address,uint24,int24,address)` | `783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118` |

**V2 and V3 `Swap` are different events with different signatures.** Matching the wrong topic0 yields zero rows, which reads like "no data" rather than a bug. Confirm the protocol version during pre-flight.

V3 `amount0`/`amount1` are **signed** (`int256`): negative = leaving the pool. Decode as signed and preserve the sign — do not `abs()` silently.

## Well-known mainnet addresses

Offer as pre-flight choices; always let the user paste their own.

| Token | Address | Decimals |
|---|---|---|
| USDC | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 6 |
| USDT | `0xdAC17F958D2ee523a2206206994597C13D831ec7` | 6 |
| DAI | `0x6B175474E89094C44Da98b954EedeAC495271d0F` | 18 |
| WETH | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` | 18 |
| WBTC | `0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599` | 8 |

| Contract | Address |
|---|---|
| Uniswap V2 Factory | `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f` |
| Uniswap V3 Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` |
| Uniswap V2 USDC-ETH pair | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` |

**Decimals differ per token and are not guessable** — USDC is 6, not 18. Either resolve via `decimals()` RPC (see [rpc-and-tokens.md](./rpc-and-tokens.md)) or use a verified constant. Assuming 18 is a common and silent correctness bug.

## Address comparison

Addresses in logs are raw 20-byte values. Two safe patterns:

```rust
// Preferred: compare bytes, no allocation
const USDC: [u8; 20] = hex_literal::hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
if log.address == USDC { /* … */ }

// If comparing as strings, both sides must be lowercase and un-prefixed
if hex::encode(&log.address) == "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" { /* … */ }
```

`hex::encode` / `Hex::encode` emit **lowercase, no `0x`**. Comparing either against a checksummed or `0x`-prefixed literal silently never matches — a top cause of "empty output". Prefer **byte** compares for filters (`log.address == USDC`). For **emitted** address/tx fields, re-prefix: `format!("0x{}", Hex::encode(…))`. `hex_literal::hex!` accepts either case.

## EVM networks

The `network:` value in the manifest selects the chain; the block type is `sf.ethereum.type.v2.Block` for all of them.

| Chain | `network:` |
|---|---|
| Ethereum mainnet | `mainnet` |
| Base | `base` |
| Arbitrum One | `arbitrum-one` |
| Polygon | `matic` |
| Optimism | `optimism` |
| BNB Smart Chain | `bsc` |

**The names are not the ones you would guess.** Polygon is `matic`, not `polygon`; Arbitrum One is `arbitrum-one`, not `arbitrum`. `substreams-dev` `references/networks.md` is the authority (sourced from The Graph Networks Registry) — check it rather than inferring from the chain's brand name.

Contract addresses are **chain-specific**: the USDC address above is mainnet-only.

## Computing a topic0 yourself

When the event is not listed here, derive it from the canonical signature rather than searching for a hash to copy:

```bash
# Foundry
cast keccak "Swap(address,uint256,uint256,uint256,uint256,address)"
```

From Solidity source, strip parameter names, `indexed` keywords, and spaces:

```solidity
event Swap(address indexed sender, uint amount0In, ..., address indexed to);
// canonical → Swap(address,uint256,uint256,uint256,uint256,address)
```

Note `uint` → `uint256` in the canonical form. Leaving it as `uint` produces a wrong hash and zero matches.
