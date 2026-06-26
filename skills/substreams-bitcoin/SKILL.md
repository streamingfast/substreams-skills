---
name: substreams-bitcoin
description: Expert knowledge for developing Bitcoin Substreams. Use when working with Bitcoin blockchain data, UTXO model, transaction parsing, address extraction, or Esplora API compatibility.
license: Apache-2.0
compatibility:
  platforms: [claude-code, cursor, vscode, windsurf]
metadata:
  version: 1.2.0
  author: PaulieB14
  documentation: https://github.com/PaulieB14/Bitcoin-Explorer-Substream
---

# Substreams Bitcoin Development Expert

Expert assistant for building Bitcoin Substreams - processing Bitcoin blockchain data with UTXO model support.

## Bitcoin vs EVM: Key Differences

Understanding the fundamental differences between Bitcoin and EVM chains is crucial:

| Aspect | Bitcoin | Ethereum/EVM |
|--------|---------|--------------|
| **Model** | UTXO (Unspent Transaction Outputs) | Account-based |
| **Balances** | Sum of unspent outputs | Stored in account state |
| **Transactions** | Consume UTXOs, create new ones | Modify account states |
| **Smart Contracts** | Script (limited) | Full Turing-complete |
| **Addresses** | Derived from scripts | 20-byte identifiers |
| **Witness Data** | SegWit (separate) | Part of transaction |

## Core Data Structures

### Block Structure

The `sf.bitcoin.v1.Block` type provides:

```rust
use substreams_bitcoin::pb::btc::v1::{Block, Transaction, Vin, Vout};

// Block fields
block.hash            // Block hash (hex string)
block.height          // Block height
block.version         // Block version
block.time            // Timestamp (Unix)
block.mediantime      // Median time
block.nonce           // Mining nonce
block.bits            // Difficulty bits
block.difficulty      // Calculated difficulty
block.size            // Block size (bytes)
block.weight          // Block weight (SegWit)
block.stripped_size   // Size without witness
block.merkle_root     // Merkle root
block.previous_hash   // Previous block hash
block.tx              // Vec<Transaction>
```

### Transaction Structure

```rust
// Transaction fields
tx.txid         // Transaction ID
tx.hash         // Transaction hash (differs from txid for SegWit)
tx.version      // Version number
tx.size         // Total size
tx.weight       // Weight (SegWit)
tx.locktime     // Lock time
tx.vin          // Vec<Vin> - inputs
tx.vout         // Vec<Vout> - outputs
```

### Input (Vin) Structure

```rust
// Input fields
vin.txid           // Previous transaction ID
vin.vout           // Previous output index
vin.coinbase       // Coinbase data (empty for non-coinbase)
vin.txinwitness    // Vec<String> - witness data
vin.sequence       // Sequence number
vin.script_sig     // Script signature
```

### Output (Vout) Structure

```rust
// Output fields
vout.value           // Value in BTC (f64)
vout.n               // Output index
vout.script_pub_key  // Script public key with:
    .hex             // Hex-encoded script
    .asm             // Assembly representation
    .r#type          // Script type (p2pkh, p2sh, p2wpkh, etc.)
    .address         // Derived address
    .req_sigs        // Required signatures
```

## Common Workflows

### Basic Block Processing

```rust
use substreams::errors::Error;
use substreams::pb::substreams::Clock;
use substreams_bitcoin::pb::btc::v1::Block;

#[substreams::handlers::map]
fn map_block_data(_clock: Clock, block: Block) -> Result<MyBlockData, Error> {
    Ok(MyBlockData {
        hash: block.hash.clone(),
        height: block.height as u64,
        tx_count: block.tx.len() as u32,
        timestamp: block.time as u64,
    })
}
```

### Transaction Processing

```rust
#[substreams::handlers::map]
fn map_transactions(_clock: Clock, block: Block) -> Result<Transactions, Error> {
    let txs: Vec<TransactionData> = block.tx.iter().map(|tx| {
        TransactionData {
            txid: tx.txid.clone(),
            is_coinbase: tx.vin.iter().any(|v| !v.coinbase.is_empty()),
            input_count: tx.vin.len() as u32,
            output_count: tx.vout.len() as u32,
        }
    }).collect();

    Ok(Transactions { transactions: txs })
}
```

### Address Extraction

Extract addresses from output scripts:

```rust
fn extract_address(vout: &Vout) -> Option<String> {
    vout.script_pub_key
        .as_ref()
        .map(|s| s.address.clone())
        .filter(|a| !a.is_empty())
}

fn extract_address_type(vout: &Vout) -> String {
    vout.script_pub_key
        .as_ref()
        .map(|s| s.r#type.clone())
        .unwrap_or_default()
}
```

### UTXO Tracking

Track UTXOs per block (within single block scope):

```rust
#[substreams::handlers::map]
fn map_utxos(_clock: Clock, block: Block) -> Result<Utxos, Error> {
    let mut utxos = Vec::new();

    for tx in &block.tx {
        for (idx, output) in tx.vout.iter().enumerate() {
            if let Some(address) = extract_address(output) {
                utxos.push(Utxo {
                    txid: tx.txid.clone(),
                    vout: idx as u32,
                    value: btc_to_sats(output.value),
                    address,
                    block_height: block.height as u32,
                });
            }
        }
    }

    Ok(Utxos { utxos })
}

/// Convert BTC to satoshis
fn btc_to_sats(btc: f64) -> u64 {
    (btc * 100_000_000.0) as u64
}
```

### Witness Data Processing

Handle SegWit witness data:

```rust
fn process_input(input: &Vin) -> InputData {
    InputData {
        txid: input.txid.clone(),
        vout: input.vout,
        is_coinbase: !input.coinbase.is_empty(),
        witness: input.txinwitness.clone(), // Already Vec<String>
        sequence: input.sequence,
    }
}

// Aggregate all witness data for a transaction
fn get_tx_witness(tx: &Transaction) -> Vec<String> {
    tx.vin
        .iter()
        .flat_map(|input| input.txinwitness.clone())
        .collect()
}
```

### Fee Estimation

Estimate transaction fees (accurate calculation requires UTXO lookup):

```rust
fn estimate_fee(tx: &Transaction) -> u64 {
    // Coinbase transactions have no fee
    if tx.vin.iter().any(|v| !v.coinbase.is_empty()) {
        return 0;
    }

    let output_total: u64 = tx.vout
        .iter()
        .map(|o| btc_to_sats(o.value))
        .sum();

    // Estimate: assume ~1.5% fee rate for typical transactions
    let estimated_input = (output_total as f64 * 1.015) as u64;
    estimated_input.saturating_sub(output_total)
}
```

## Bitcoin Address Types

| Type | Prefix | Script Type | Description |
|------|--------|-------------|-------------|
| P2PKH | 1 | `pubkeyhash` | Legacy pay-to-public-key-hash |
| P2SH | 3 | `scripthash` | Pay-to-script-hash (multisig, etc.) |
| P2WPKH | bc1q | `witness_v0_keyhash` | Native SegWit v0 |
| P2WSH | bc1q | `witness_v0_scripthash` | SegWit script hash |
| P2TR | bc1p | `witness_v1_taproot` | Taproot (SegWit v1) |

## Manifest Configuration

### Basic Bitcoin Substream

```yaml
specVersion: v0.1.0
package:
  name: my-bitcoin-substreams
  version: v1.2.0
  url: https://github.com/user/repo
  description: Bitcoin data processor

network: bitcoin

protobuf:
  files:
    - my_types.proto
  importPaths:
    - ./proto

binaries:
  default:
    type: wasm/rust-v1
    file: target/wasm32-unknown-unknown/release/my_bitcoin_substreams.wasm

modules:
  - name: map_blocks
    kind: map
    initialBlock: 0
    inputs:
      - source: sf.substreams.v1.Clock
      - source: sf.bitcoin.v1.Block
    output:
      type: proto:my.types.BlockData
```

### Cargo.toml for Bitcoin

```toml
[package]
name = "my_bitcoin_substreams"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
substreams = "0.0.21"
substreams-bitcoin = "1.0.0"
prost = "0.11.9"
prost-types = "0.11.9"

[build-dependencies]
prost-build = "0.11.9"
```

## Esplora API Compatibility

When building Esplora-compatible output, map to these structures:

### Block Response
```json
{
  "id": "block_hash",
  "height": 800000,
  "version": 536870912,
  "timestamp": 1690168629,
  "tx_count": 3721,
  "size": 1621583,
  "weight": 3993561,
  "merkle_root": "...",
  "previousblockhash": "...",
  "difficulty": 53911173001054.59
}
```

### Transaction Response
```json
{
  "txid": "...",
  "version": 2,
  "locktime": 0,
  "size": 225,
  "weight": 573,
  "fee": 4500,
  "vin": [...],
  "vout": [...],
  "status": {
    "confirmed": true,
    "block_height": 800000,
    "block_hash": "...",
    "block_time": 1690168629
  }
}
```

## Important Bitcoin-Specific Considerations

### 1. UTXO Model Implications

- **No account balances**: Must track UTXOs to compute balances
- **Transaction fees**: Input sum - Output sum (requires UTXO lookup)
- **Address reuse**: Same address can have multiple UTXOs

### 2. SegWit Transactions

- `txid` != `hash` for SegWit transactions
- Witness data is separate from script signature
- Weight = 3 * base_size + total_size

### 3. Coinbase Transactions

- First transaction in each block
- `vin[0].coinbase` contains coinbase data
- No previous outputs to reference
- Mining reward + transaction fees

### 4. Script Types

Different output script types require different parsing:
- P2PKH: `OP_DUP OP_HASH160 <pubkeyhash> OP_EQUALVERIFY OP_CHECKSIG`
- P2SH: `OP_HASH160 <scripthash> OP_EQUAL`
- P2WPKH: `OP_0 <keyhash>`
- P2TR: `OP_1 <pubkey>`

## Running Bitcoin Substreams

```bash
# Authenticate
substreams auth

# Run against Bitcoin endpoint
substreams run -e bitcoin \
  my-substream.spkg \
  map_blocks -s 800000 -t +100

# GUI mode
substreams gui -e bitcoin \
  my-substream.spkg \
  map_blocks -s 800000
```

## Resources

- [Bitcoin Protocol Documentation](https://en.bitcoin.it/wiki/Protocol_documentation)
- [Esplora API Reference](https://github.com/Blockstream/esplora/blob/master/API.md)
- [substreams-bitcoin Crate](https://crates.io/crates/substreams-bitcoin)
- [Bitcoin Esplora Enhanced](https://github.com/PaulieB14/Bitcoin-Explorer-Substream)
- [Substreams Documentation](https://substreams.streamingfast.io)
