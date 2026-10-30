# Performance Guidelines

Source: the Substreams docs "Performance Guidelines". During a backfill the engine spends time reading blocks, running module code, reading and writing cached module output, and building stores. Each guideline reduces one of those costs.

## 1. Keep the number of modules small

- Every module copies its inputs into the WASM engine and writes and reads its output through the cache **on every block**.
- Put extraction logic in **one module that reads the block**, not one module per event, call, or instruction. The consumer experience is also simpler.
- Split a module only when **other packages will reuse its output**. Do not split just for composability.

## 2. Reduce data size early

- Read the block in as few modules as possible (ideally one) and output only the fields you need.
- Call `substreams::skip_empty_output()` when a module has nothing to emit for a block. A module whose inputs are all skipped does not run for that block. Only the clock input is ignored in this check; a protobuf message with only default values counts as empty.

```rust
#[substreams::handlers::map]
fn map_trades(block: Block) -> Result<Trades, Error> {
    substreams::skip_empty_output();
    let trades = extract_trades(&block);
    Ok(Trades { trades })
}
```

## 3. Keep stores few and independent

- A store that reads another store, directly or through a map, is built in a **later stage**. Each stage is another full pass over the blocks of every segment, and merging is sequential. Avoid chains of stores; if a map already extracts the data, read the map.
- Keep stores small and delete keys you no longer need. Every segment loads the full store, and stores have a size limit. For large datasets use a **Foundational Store**.
- Before shipping a new version of a package, read [updating-packages.md](./updating-packages.md): a code change gives **every store from the same binary** a new hash, and each is backfilled from its initial block.

## 4. Use block filters with foundational indexes

A block filter skips blocks with nothing of interest; when the index is cached, whole segments without matches are skipped. Use the index modules of foundational packages; write your own only when several of your modules filter on it. See [block-filtering.md](./block-filtering.md).

## 5. Set `initialBlock` as late as possible

The first block that can contain data for your module (e.g. the contract/program deployment block). Matters most for stores, which always backfill from their initial block.

## 6. Write efficient Rust

- **Borrow, don't clone.** Cloning transactions or blocks copies all their content.
- **Check before decoding.** Compare the raw address, event signature, or program ID first.
- **Keep data as bytes until the end.** Encode hex/base58 only in the last module (e.g. `db_out`). Base58 is slow; hex doubles the data size.
- **Don't log in loops.** Each log call formats and copies a string even if no one reads it.
- **Use `int64` / `float64` stores when values fit.** `bigint` / `bigdecimal` cost more to encode, decode, and add.

## Chain-specific

### EVM

Start from `sf.ethereum.type.v2.Block` with a `blockFilter` on `ethereum_common:index_events` / `index_calls`:

```yaml
imports:
  ethereum_common: ethereum_common@v0.3.3

modules:
  - name: map_trades
    kind: map
    inputs:
      - source: sf.ethereum.type.v2.Block
    blockFilter:
      module: ethereum_common:index_events
      query:
        string: evt_addr:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
    output:
      type: proto:my.types.v1.Trades
```

`eth_call`s are network requests that **block processing** until they return, so they slow a run more than computation does: call only when you need the result (e.g. the first time you see a token), prefer execution-trace data (e.g. `storage_changes`), batch calls within a block with `RpcBatch`, and read static data such as token decimals from the ERC20 Token Metadata Foundational Store where it is deployed. Details: `substreams-ethereum` → `references/rpc-and-tokens.md`.

### Solana

Read `sf.solana.type.v1.Block` directly and filter with `solana_common:program_ids_without_votes` (`program:<id>` keys). StreamingFast's Solana endpoints no longer serve vote transactions, so `blocks_without_votes` only adds a module to the pipeline. Check the program ID before decoding instruction data.

## Measuring

Module output is cached only in `--production-mode`, and a code change alters the module hash (cold cache) — compare runs with the same cache state. See `substreams-testing` → `references/performance-testing.md`.
