# Integration Testing Guide

Integration tests validate modules against **real (or fixture) blockchain data** and the **Substreams engine** (WASM + host).

## What belongs here

| Style | When | Offline? |
|---|---|---|
| Multi-block `map!` over fixtures | Pipeline logic, no engine | Yes |
| `substreams run` + JSONL / golden | Full execution path | Needs network or local endpoint |
| `substreams run --test-file` | jq assertions on module outputs | Same as run |
| Optional `Command` in Rust | Automate CLI in CI | Mark `#[ignore]` by default |

There is **no** `substreams::test_utils` crate and **no** official in-process `TestStore`.

## Multi-block fixture tests (still unit-shaped)

```rust
#[test]
fn transfers_across_fixtures() {
    let blocks = [
        load_block_b64("tests/fixtures/eth_17000000.binpb.b64"),
        load_block_b64("tests/fixtures/eth_17000001.binpb.b64"),
    ];
    let mut total = 0usize;
    for block in blocks {
        let out = substreams::testing::map!(map_transfers(block)).unwrap();
        total += out.transfers.len();
    }
    assert!(total > 0);
}
```

Use protobuf fixtures from Firecore (see [firecore-tools.md](./firecore-tools.md)). Assert against **known ground truth** for those block numbers when possible.

## CLI integration (`substreams run`)

```bash
substreams build

# Smoke: non-empty JSONL
substreams run -s 17000000 -t +100 map_transfers \
  --network mainnet -o jsonl | tee /tmp/out.jsonl
test -s /tmp/out.jsonl

# Golden: normalize then diff (project-specific strip of envelopes / hex case)
# diff -u golden/map_transfers_17000000_100.jsonl /tmp/out.normalized.jsonl
```

**Auth:** `substreams auth` or `SUBSTREAMS_API_TOKEN` / `SUBSTREAMS_API_KEY` (data plane — not Portal JWT).

**Manifest hygiene for tests:**

- Set `initialBlock` near `-s` so stores do not backfill from genesis
- Prefer a real `network:` and documented module name
- Start with `+10` / `+100` blocks; widen after green

### Production mode parity

```bash
substreams run -s 17000000 -t +200 map_transfers --network mainnet -o jsonl > /tmp/dev.jsonl
substreams run -s 17000000 -t +200 map_transfers --network mainnet -o jsonl \
  --production-mode > /tmp/prod.jsonl
# Compare after any project-specific normalization
```

Mismatch can indicate non-determinism, clock dependence, or host differences — investigate before shipping.

## `--test-file` assertions

The CLI embeds a test runner (`tools/test` in the substreams binary). Supported extensions: **`.yaml`**, **`.jsonl`**, **`.csv`**.

### YAML shape

```yaml
tests:
  - module: map_transfers
    block: 17000000
    path: .transfers | length
    expect: "0"
    op: float
  - module: map_transfers
    block: 17000000
    path: .transfers[0].contract
    expect: "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
  - module: store_balances
    block: 17000005
    path: select(.key == "usdc:0xalice") | .new
    expect: "1000000"
```

| Field | Meaning |
|---|---|
| `module` | Module name as in the manifest |
| `block` | Absolute block number (must appear in the run range) |
| `path` | **gojq** expression over the module’s JSON output |
| `expect` | Expected value (string form) |
| `op` | Optional comparator (`float`, …) |
| `args` | Optional comparator args (e.g. `error=2`) |

### JSONL shape

One JSON object per line with the same fields:

```json
{"module":"map_transfers","block":17000000,"path":".transfers|length","expect":"3","op":"float"}
```

### CSV shape

Columns: `module,block,path,expect[,op[,args]]`

### Run

```bash
substreams run substreams.yaml map_transfers \
  -s 17000000 -t +50 \
  --network mainnet \
  --test-file tests/assertions.yaml \
  --test-verbose
```

**Notes:**

- Store modules are evaluated from **debug store outputs** when the runner receives them — select modules / debug flags as needed for your CLI version so store tests actually fire
- Failed assertions fail the run; use `--test-verbose` while drafting paths
- Prefer lowercase hex in `expect` if your module emits lowercase

## Rust wrappers around the CLI

Keep network work out of the default suite:

```rust
use std::process::Command;

#[test]
#[ignore = "requires substreams auth + network"]
fn e2e_map_transfers_smoke() {
    let output = Command::new("substreams")
        .args([
            "run",
            "-s",
            "17000000",
            "-t",
            "+10",
            "map_transfers",
            "--network",
            "mainnet",
            "-o",
            "jsonl",
        ])
        .output()
        .expect("substreams CLI");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.lines().count() > 0);
}
```

```bash
cargo test -- --ignored
```

## Ground-truth / expected files

For high-value blocks, store expected summaries next to fixtures:

```json
{
  "block_number": 17000000,
  "expected_transfer_count_min": 1,
  "must_include_contracts": ["a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"]
}
```

Load in tests and assert counts / membership rather than brittle full-output equality (unless you maintain golden JSONL deliberately).

## Reorgs

True reorg / undo behavior is engine-owned. Integration options:

1. Document that stores are delta-based and rely on Substreams reorg handling in sinks
2. Unit-test pure “apply delta / invert delta” helpers if you implement them
3. Do **not** invent a fake `TestStore` that claims to simulate undo without the runtime

If you need sink-level reorg tests, use `substreams-sink` / deploy-local skills with a real cursor stream.

## CI recommendations

```yaml
jobs:
  unit:
    # cargo test --lib  (no secrets)

  integration:
    if: github.event_name == 'push'  # or manual / nightly
    env:
      SUBSTREAMS_API_TOKEN: ${{ secrets.SUBSTREAMS_API_TOKEN }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - name: Install substreams CLI
        run: |
          # pin a release URL appropriate for the runner OS/arch
          curl -sSL -o substreams.tar.gz \
            "https://github.com/streamingfast/substreams/releases/download/v1.17.2/substreams_linux_x86_64.tar.gz"
          tar -xzf substreams.tar.gz substreams
          sudo mv substreams /usr/local/bin/
      - run: substreams build
      - run: |
          substreams run -s 17000000 -t +20 map_transfers --network mainnet -o jsonl
          substreams run -s 17000000 -t +20 map_transfers --network mainnet \
            --test-file tests/assertions.yaml
```

Pin CLI versions; do not scrape `latest` without checksums in production orgs.

## Checklist

- [ ] Fixtures committed or generated in a documented script
- [ ] `initialBlock` compatible with test start
- [ ] Default `cargo test` green offline
- [ ] At least one `substreams run` smoke path documented
- [ ] Optional `--test-file` or golden JSONL for regressions
- [ ] Secrets only on intentional jobs

## See also

- [Unit testing](./unit-testing.md)
- [Performance testing](./performance-testing.md)
- [Firecore tools](./firecore-tools.md)
- [Official testing docs](https://docs.substreams.dev/reference-material/development-tools/testing)
