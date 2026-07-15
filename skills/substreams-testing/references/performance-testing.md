# Performance Testing Guide

Measure Substreams module cost before large backfills. Prefer **real heavy blocks** over synthetic empty ones.

## What to measure

| Layer | Question | Tool |
|---|---|---|
| Pure map logic | µs per block in-process | Criterion + `map!` |
| Full engine | blocks/s, wall time | `time substreams run` |
| Parallel path | prod vs dev parity + speed | `--production-mode` |
| Cost drivers | skipped vs processed blocks | `blockFilter` / indexes (`substreams-dev`) |

**Biggest win is usually skipping blocks**, not micro-optimizing a map that already only sees filtered data.

## Criterion benchmarks

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "map_events"
harness = false
```

```rust
// benches/map_events.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use substreams::testing;

fn bench_map_events(c: &mut Criterion) {
    let block = load_heavy_fixture(); // real block protobuf fixture

    c.bench_function("map_events_heavy_block", |b| {
        b.iter(|| {
            testing::map!(map_events(black_box(block.clone())))
        })
    });
}

criterion_group!(benches, bench_map_events);
criterion_main!(benches);
```

```bash
cargo bench
cargo bench -- --save-baseline before
# change code…
cargo bench -- --baseline before
```

**Tips:**

- Clone cost of a full `Block` dominates some benches — measure apply path carefully; prefer reusing one fixture and only cloning when the handler consumes the block by value
- Avoid cloning inside handlers in production code (`substreams-dev` performance section)
- Benchmark the **WASM-free** path with `map!`; still time `substreams run` for engine overhead

## CLI timing

```bash
substreams build

echo "development mode"
time substreams run -s 17000000 -t +1000 map_events \
  --network mainnet -o jsonl > /tmp/dev.jsonl

echo "production mode"
time substreams run -s 17000000 -t +1000 map_events \
  --network mainnet -o jsonl --production-mode > /tmp/prod.jsonl
```

Check:

1. Exit code 0
2. Outputs equivalent after normalization
3. Wall time and whether prod is faster (often yes for large ranges)

### Resource pressure

```bash
# Optional: observe RSS while running (macOS sample)
# /usr/bin/time -l substreams run …

# Fail CI if process is OOM-killed — inspect stderr for "killed", "out of memory"
```

Large store state and excessive logging hurt more than tight loops. Reduce store keys, avoid `println!` in handlers, and add indexes before buying a bigger machine.

## Performance checklist for agents

1. Confirm a **block index + `blockFilter`** if the contract/program is sparse
2. Set `initialBlock` to the consumer’s start (not protocol genesis) for fair timings
3. Unit-bench map logic on a known heavy fixture
4. Time `+100` then `+1000` with and without `--production-mode`
5. Compare JSONL outputs for determinism
6. Only then optimize Rust (allocations, clones, regexes)

## Anti-patterns

| Anti-pattern | Prefer |
|---|---|
| Timing only empty synthetic blocks | Heavy real fixtures |
| Claiming store “TPS” via a fake `TestStore` | CLI / sink metrics |
| Benchmarks that include network download every iter | Cached fixtures |
| Optimizing before measuring | Criterion + `time` |
| Ignoring index filters | `substreams-dev` block filtering guide |

## CI (optional nightly)

```yaml
# sketch only — needs auth secrets and time budget
- name: Perf smoke
  run: |
    time substreams run -s 17000000 -t +500 map_events \
      --network mainnet --production-mode -o clock
```

Do not gate every PR on multi-minute backfills.

## See also

- [Unit testing](./unit-testing.md) — `map!` benches inputs
- [Integration testing](./integration-testing.md) — production mode parity
- [substreams-dev](../../substreams-dev/SKILL.md) — cloning, indexes, `initialBlock`
- [Criterion book](https://bheisler.github.io/criterion.rs/book/)
