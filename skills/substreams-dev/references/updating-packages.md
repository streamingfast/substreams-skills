# Updating a Published Package Without Re-Processing Stores

Source: the Substreams docs "Updating Your Substreams". Every module whose **hash** changed is processed again from its `initialBlock`. For a map that costs only the blocks you request; for a store it is a **full backfill** before any module reading it can emit.

## What changes a module hash

- The content and `type` of the **whole WASM file** (not just the module's function). Any change to the crate — including code the module never calls, or a dependency version — changes the hash of **every module built from that file**.
- The Rust function name, the module `kind`, and the `initialBlock` (resolved from inputs when not set)
- The inputs: order and type, `source` type, `params` value, `foundational-store` identifier
- The `blockFilter`: the index module's hash and the query
- The hash of every module it reads from, directly or indirectly

**Not** part of the hash: module name, `doc`, package name and version.

Compare versions with `substreams info old.spkg` vs `substreams info substreams.yaml` (the `Hash:` line per module).

## Keep a module's hash: run the previous binary

### Option 1 (recommended): import the previous package and `use` the module

```yaml
imports:
  v0_1_0: ./polymarket-v0.1.0.spkg

modules:
  - name: store_markets
    use: v0_1_0:store_markets
    initialBlock: 40000000   # MUST equal the previous version's value
```

`use` copies `kind` (+ `updatePolicy`/`valueType`), `output`, the binary and function, `inputs` (when you set none), and `blockFilter` (when you set none; `blockFilter: {}` clears it). It does **not** copy `initialBlock` or `doc`. A `use` module cannot set `kind`, `output`, `updatePolicy`, `valueType`, or `binary`. If you set `inputs` they must match the used module's one by one (same count/order, same source type, same store `mode`, same map output type, `params` stays `params`).

### Option 2: keep the previous WASM file

```yaml
binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/polymarket.wasm
  v0_1_0:
    type: wasm/rust-v1
    file: ./previous/polymarket-v0.1.0.wasm

modules:
  - name: store_markets
    kind: store
    binary: v0_1_0
    initialBlock: 40000000
    updatePolicy: set
    valueType: proto:polymarket.v1.Market
    inputs:
      - source: sf.ethereum.type.v2.Block
```

Rebuilding old source can produce a different file — extract it from the published package instead:

```bash
substreams tools extract-wasm ./polymarket-v0.1.0.spkg ./previous/polymarket-v0.1.0.wasm   # --module <name> if several binaries
substreams tools unpack ./polymarket-v0.1.0.spkg    # manifest + every binary -> polymarket-unpacked/
```

Every module the store reads from, directly or indirectly, must also use the previous binary.

### Checklist (both options)

- `initialBlock` equals the previous version's (omitted → resolved from inputs → possibly different hash)
- Inputs identical (with `use`, omit `inputs` to copy them)
- `params` value unchanged
- Run `substreams info` on old and new and compare `Hash:` before deploying
- If you changed the reused module's output message, update its readers or don't reuse it

## Bug fix in a map, store is fine

`v0.1.1` takes `store_markets` from `v0.1.0` via `use` (same `initialBlock`) and defines `map_trades` with the new code. The store keeps its hash and is read from cache; only `map_trades` is reprocessed.

## New protocol version (new contract, new format)

Import the old package, import its `map_trades`, add a **new** store starting at the upgrade block and a new output map that forwards old trades until the old contract stops being used:

```yaml
modules:
  - name: store_markets_v2
    kind: store
    initialBlock: 80000000          # only backfills after the upgrade
    updatePolicy: set
    valueType: proto:polymarket.v2.Market
    inputs: [{ source: sf.ethereum.type.v2.Block }]

  - name: map_trades_v2
    kind: map
    initialBlock: 40000000          # MUST be set: inputs start at different blocks
    inputs:
      - source: sf.ethereum.type.v2.Block
      - map: v0_1_0:map_trades
      - store: store_markets_v2
    output: { type: proto:polymarket.v2.Trades }
```

Hard-code the cut-over block (`V1_END_BLOCK`) **before** deploying: changing it later changes the WASM file and therefore `store_markets_v2`'s hash.
