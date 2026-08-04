# Common Substreams Patterns

Collection of proven patterns and best practices for Substreams development.

> **EVM-heavy examples below.** For dedicated contract workflows use the **`substreams-ethereum`** skill. For Solana use **`substreams-solana`**.

> **Note:** Code examples below assume the following imports unless stated otherwise:
> ```rust
> use substreams::errors::Error;
> use substreams::prelude::*;
> use substreams::Hex;
> use substreams_ethereum::pb::eth::v2::{Block, TransactionTrace};
> ```

## Event Extraction

Not owned by this skill. EVM event extraction — ABI codegen (`substreams init`, `build.rs`), the `use substreams_ethereum::Event;` trait import that `.match_and_decode()` requires, topic0 matching, and indexed-vs-non-indexed payload decoding — belongs to the **`substreams-ethereum`** skill (see its "Canonical loop" section and `references/abi-codegen.md`). For Solana instruction decoding use **`substreams-solana`**.

## Store Aggregation Patterns

### Factory Contract Pattern

The primary use case for stores in Substreams is tracking factory contracts and their created instances:

```rust
#[substreams::handlers::store]
pub fn store_factory_contracts(
    events: FactoryEvents,
    store: StoreSetString,
) {
    for event in events.items {
        match event.event_type.as_str() {
            "PairCreated" => {
                let pair_address = event.pair_address;
                let key = format!("pair:{}", pair_address);
                
                let pair_info = PairInfo {
                    token0: event.token0,
                    token1: event.token1,
                    factory: event.factory_address,
                    created_at: event.block_num,
                };
                
                let serialized = serde_json::to_string(&pair_info).unwrap();
                store.set(0, &key, &serialized);
            }
            "PoolCreated" => {
                let pool_address = event.pool_address;
                let key = format!("pool:{}", pool_address);
                
                let pool_info = PoolInfo {
                    token0: event.token0,
                    token1: event.token1,
                    fee: event.fee,
                    factory: event.factory_address,
                    created_at: event.block_num,
                };
                
                let serialized = serde_json::to_string(&pool_info).unwrap();
                store.set(0, &key, &serialized);
            }
            _ => {}
        }
    }
}
```

### Registry Pattern

Track contract registrations and metadata:

```rust
#[substreams::handlers::store]
pub fn store_contract_registry(
    events: RegistryEvents,
    store: StoreSetString,
) {
    for event in events.items {
        match event.event_type.as_str() {
            "ContractRegistered" => {
                let key = format!("contract:{}", event.contract_address);
                
                let contract_info = ContractInfo {
                    name: event.name,
                    version: event.version,
                    owner: event.owner,
                    registered_at: event.block_num,
                };
                
                let serialized = serde_json::to_string(&contract_info).unwrap();
                store.set(0, &key, &serialized);
            }
            _ => {}
        }
    }
}
```

### Best Practices for Store Usage

**Recommended for Substreams:**
- Factory contract tracking
- Contract registry management
- Dynamic contract discovery
- Metadata storage for enrichment

**Recommended to handle outside Substreams:**
- Complex aggregations (use external databases)
- Time-series data (use specialized time-series databases)
- Analytics and reporting (use data warehouses)
- Real-time dashboards (use streaming analytics platforms)

## Multi-Module Composition

### Producer-Consumer Pattern

```yaml
# Producer module
- name: map_raw_events
  kind: map
  inputs:
    - source: sf.ethereum.type.v2.Block
  output:
    type: proto:events.RawEvents

# Consumer modules
- name: store_event_counts
  kind: store
  updatePolicy: add
  valueType: int64
  inputs:
    - map: map_raw_events

- name: store_event_metadata
  kind: store
  updatePolicy: set
  valueType: string
  inputs:
    - map: map_raw_events

- name: index_active_blocks
  kind: blockIndex
  inputs:
    - map: map_raw_events
  output:
    type: proto:sf.substreams.index.v1.Keys
```

### Enrichment Pipeline

```rust
// Step 1: Extract basic events
#[substreams::handlers::map]
pub fn map_basic_events(block: Block) -> Result<BasicEvents, Error> {
    // Extract events without enrichment
}

// Step 2: Build metadata store
#[substreams::handlers::store]
pub fn store_metadata(events: BasicEvents, store: StoreSetString) {
    // Store metadata for enrichment
}

// Step 3: Enrich events
#[substreams::handlers::map]
pub fn map_enriched_events(
    events: BasicEvents,
    metadata: StoreGetString,
) -> Result<EnrichedEvents, Error> {
    let mut enriched = EnrichedEvents::default();
    
    for event in events.items {
        let metadata_key = format!("meta:{}", event.contract);
        let metadata = metadata.get_last(&metadata_key);
        
        enriched.items.push(EnrichedEvent {
            event: Some(event),
            metadata,
        });
    }
    
    Ok(enriched)
}
```

## Parameterized Modules

### Contract-Specific Processing

```rust
#[substreams::handlers::map]
pub fn map_contract_events(
    params: String,
    block: Block,
) -> Result<ContractEvents, Error> {
    let target_contract = params.to_lowercase();
    let mut events = ContractEvents::default();
    
    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            let contract_addr = Hex::encode(&log.address).to_lowercase();
            if contract_addr == target_contract {
                events.items.push(extract_event(log, trx, &block));
            }
        }
    }
    
    Ok(events)
}
```

Usage:
```bash
substreams run map_contract_events \
  -p map_contract_events=0xa0b86a33e6842a82e50c9c82c95846c26c7b3b96
```

### Multi-Parameter Processing

```rust
#[derive(serde::Deserialize)]
struct Params {
    contracts: Vec<String>,
    min_value: u64,
    include_failed: bool,
}

#[substreams::handlers::map]
pub fn map_filtered_events(
    params: String,
    block: Block,
) -> Result<FilteredEvents, Error> {
    let params: Params = serde_json::from_str(&params)?;
    let mut events = FilteredEvents::default();
    
    for trx in block.transactions() {
        // Skip failed transactions if not included
        if !params.include_failed && trx.status() != substreams_ethereum::pb::eth::v2::TransactionTraceStatus::Succeeded {
            continue;
        }
        
        for (log, _call) in trx.logs_with_calls() {
            let contract = Hex::encode(&log.address).to_lowercase();
            
            if params.contracts.contains(&contract) {
                if let Some(event) = extract_and_filter_event(log, trx, &block, &params) {
                    events.items.push(event);
                }
            }
        }
    }
    
    Ok(events)
}
```

Usage:
```bash
substreams run map_filtered_events \
  -p 'map_filtered_events={"contracts":["0x123...","0x456..."],"min_value":1000,"include_failed":false}'
```

## Dynamic Data Sources

### Registry-Based Processing

```rust
#[substreams::handlers::map]
pub fn map_dynamic_contracts(
    block: Block,
    registry_store: StoreGetString,
) -> Result<DynamicEvents, Error> {
    let mut events = DynamicEvents::default();
    
    // Get list of active contracts from registry
    let active_contracts = get_active_contracts(&registry_store, block.number);
    
    for trx in block.transactions() {
        for (log, _call) in trx.logs_with_calls() {
            let contract = Hex::encode(&log.address);
            
            if active_contracts.contains(&contract) {
                events.items.push(extract_event(log, trx, &block));
            }
        }
    }
    
    Ok(events)
}

fn get_active_contracts(
    store: &StoreGetString,
    block_num: u64,
) -> HashSet<String> {
    let mut contracts = HashSet::new();
    
    // Query registry for contracts active at this block
    if let Some(registry_data) = store.get_last("active_contracts") {
        if let Ok(contract_list) = serde_json::from_str::<Vec<String>>(&registry_data) {
            contracts.extend(contract_list);
        }
    }
    
    contracts
}
```

### Factory Pattern

Populating the registry store that the pattern above reads from: see [Factory Contract Pattern](#factory-contract-pattern).

## Error Handling Patterns

### Fail-Fast Approach

The recommended approach is to fail fast when data doesn't align with expectations:

```rust
#[substreams::handlers::map]
pub fn map_strict_events(block: Block) -> Result<Events, Error> {
    let mut events = Events::default();
    
    for trx in block.transactions() {
        // Fail fast on any data inconsistency
        let trx_events = process_transaction_strict(trx)?;
        events.items.extend(trx_events);
    }
    
    Ok(events)
}

fn process_transaction_strict(trx: &TransactionTrace) -> Result<Vec<Event>, Error> {
    let mut events = Vec::new();
    
    for (log, _call) in trx.logs_with_calls() {
        // Validate data integrity before processing
        if log.topics.is_empty() {
            return Err(anyhow::anyhow!("Log missing topics at tx {}", Hex::encode(&trx.hash)));
        }
        
        if log.data.len() % 32 != 0 {
            return Err(anyhow::anyhow!("Invalid log data length at tx {}", Hex::encode(&trx.hash)));
        }
        
        // Process with strict validation
        events.push(extract_event_strict(log, trx)?);
    }
    
    Ok(events)
}
```

### Validation Patterns

```rust
fn validate_transfer(transfer: &Transfer) -> Result<(), Error> {
    if transfer.amount.is_empty() {
        return Err(anyhow::anyhow!("Empty amount"));
    }
    
    if transfer.from == transfer.to {
        return Err(anyhow::anyhow!("Self-transfer"));
    }
    
    if BigInt::from_str(&transfer.amount)?.is_negative() {
        return Err(anyhow::anyhow!("Negative amount"));
    }
    
    Ok(())
}
```

## Performance Optimization Patterns

### Efficient Filtering

```rust
#[substreams::handlers::map]
pub fn map_filtered_efficiently(block: Block) -> Result<Events, Error> {
    let mut events = Events::default();
    
    // Pre-filter transactions
    let relevant_transactions: Vec<_> = block
        .transactions()
        .filter(|trx| {
            trx.logs_with_calls().any(|(log, _call)| {
                !log.topics.is_empty() &&
                TARGET_SIGNATURES.contains(&log.topics[0])
            })
        })
        .collect();

    for trx in &relevant_transactions {
        for (log, _call) in trx.logs_with_calls() {
            if is_target_event(log) {
                events.items.push(extract_event(log, trx, &block));
            }
        }
    }

    Ok(events)
}
```

### Batch Processing

```rust
#[substreams::handlers::store]
pub fn store_batch_updates(
    events: Events,
    store: StoreAddBigInt,
) {
    let mut updates: HashMap<String, BigInt> = HashMap::new();
    
    // Batch updates by key
    for event in events.items {
        let key = format!("token:{}", event.token);
        let amount = BigInt::from_str(&event.amount).unwrap_or_default();
        
        *updates.entry(key).or_insert_with(BigInt::zero) += amount;
    }
    
    // Apply batched updates
    for (key, total_amount) in updates {
        store.add(0, &key, &total_amount);
    }
}
```

## Database Sinks

Not owned by this skill. The `db_out` module, the `Tables` API from `substreams-database-change`, and Database Changes (CDC, Postgres only) vs From-proto (all ClickHouse) mode selection belong to the **`substreams-sql`** skill.

Worth knowing when designing the graph above: the SQL sink (`substreams sink postgres`) supports **delta operations** (`add`, `max`, `min`, …) applied atomically by the database, which can replace store modules for many chain-wide aggregations — no replay from initial block, no read-modify-write, no store dependency. Weigh that against the store patterns above before adding a store.

## Testing

Not owned by this skill. Unit tests via `substreams::testing` (`map!`, `clock`), real block fixtures through firecore, `substreams run` / `--test-file` integration and golden checks, and CI patterns belong to the **`substreams-testing`** skill. Note it prefers **real protobuf block fixtures** over hand-rolled mock blocks, and that there is no official store mock.
