# FireCore Tools for Substreams Testing

Use Firehose / Firecore CLIs to **fetch real blocks** for fixtures and manual inspection. Decode into chain protobuf types in tests — there is no separate “firehose Rust client crate” required for typical unit tests.

## Install and auth

- **firecore**: [firehose-core releases](https://github.com/streamingfast/firehose-core/releases)
- **substreams** CLI: [substreams releases](https://github.com/streamingfast/substreams/releases)

Data-plane auth (streaming endpoints):

```bash
substreams auth
# or export SUBSTREAMS_API_TOKEN=… / SUBSTREAMS_API_KEY=…
```

Firecore tools also accept Firehose token env vars (see `firecore tools firehose-single-block-client --help`), commonly:

- `FIREHOSE_API_TOKEN` / `FIREHOSE_API_KEY` (names configurable via flags)

This is **not** StreamingFast Portal admin auth (`thegraph-market-api`).

## Commands that matter for testing

### Single block (preferred fixtures)

```bash
# Protobuf bytes as base64 — decode with prost in Rust tests
firecore tools firehose-single-block-client \
  mainnet.eth.streamingfast.io:443 17000000 \
  -o bytes --bytes-encoding=base64 \
  > src/testdata/eth_mainnet_17000000.binpb.b64

# JSON for human inspection (not always ideal for prost re-decode)
firecore tools firehose-single-block-client \
  mainnet.eth.streamingfast.io:443 17000000 \
  -o json
```

Block selector forms: `{block_num}`, `{block_num}:{block_id}`, or cursor (see CLI help).

### Block range stream

```bash
# Network id / alias or host; args after -- are start and stop (+N relative ok)
firecore tools firehose-client mainnet -o json -- 17000000 +100
firecore tools firehose-client mainnet -o text -- -1   # head
```

Global flags of interest:

| Flag | Use |
|---|---|
| `-o, --output` | `text`, `json`, `jsonl`, `protojson`, `protojsonl`, `bytes` |
| `--bytes-encoding` | `hex` (default), `base58`, `base64` |

```bash
firecore tools --help
firecore tools firehose-client --help
firecore tools firehose-single-block-client --help
```

### Head block without firecore

From `substreams-dev`: foundational `map_clocks` with `-s -1` (do not combine `-s -1` with relative `+N` stop). Prefer **spkg URL** forms documented there.

## Loading fixtures in Rust (Ethereum)

```rust
use prost::Message;
use substreams_ethereum::pb::eth::v2::Block;

pub fn load_block_b64(path: &str) -> Block {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let b64 = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"));
    let bytes = STANDARD.decode(b64.trim()).expect("base64 decode");
    Block::decode(bytes.as_slice()).expect("decode Block protobuf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams::testing;

    #[test]
    fn fixture_decodes() {
        let block = load_block_b64("src/testdata/eth_mainnet_17000000.binpb.b64");
        assert_eq!(block.number, 17_000_000);
        let _ = testing::map!(map_events(block));
    }
}
```

**Solana / other chains:** same idea — fetch the chain’s Firehose block type and `Message::decode` into the `substreams_*` protobuf struct your handler uses.

## Fixture workflow script

```bash
#!/usr/bin/env bash
set -euo pipefail

ENDPOINT="${ENDPOINT:-mainnet.eth.streamingfast.io:443}"
BLOCK="${1:?block number}"
OUT_DIR="${OUT_DIR:-src/testdata}"
mkdir -p "$OUT_DIR"

OUT="$OUT_DIR/eth_mainnet_${BLOCK}.binpb.b64"
firecore tools firehose-single-block-client "$ENDPOINT" "$BLOCK" \
  -o bytes --bytes-encoding=base64 \
  > "$OUT"
echo "wrote $OUT ($(wc -c < "$OUT") bytes)"
```

Commit a **small** set of blocks you assert on. Large ranges belong in optional download scripts, not the git tree.

## JSONL ranges — when to use

Use `firehose-client … -o jsonl` to:

- Browse activity and pick interesting blocks
- Draft golden expectations for `substreams run -o jsonl`
- Spot-check event presence before writing unit builders

Avoid re-hydrating full `Block` structs from Firehose JSON unless you control the schema mapping — prefer **bytes/base64 protobuf** for Rust unit tests.

## Common pitfalls

| Pitfall | Fix |
|---|---|
| Auth errors | `substreams auth` or correct API token env for the endpoint |
| Empty fixture file | Check endpoint, block number, and compression flags |
| Decode errors | Ensure output is raw block protobuf (`-o bytes`), not a JSON wrapper |
| Wrong chain package | Ethereum fixtures → `substreams_ethereum::pb::eth::v2::Block` |
| Portal token on Firehose | Use data-plane credentials, not Portal Bearer from `thegraph-market-api` |
| Huge fixtures in git | Keep one block; generate ranges in CI cache if needed |

## Relation to Substreams testing layers

1. **Fetch** with firecore (this doc)
2. **Unit-test handlers** with `map!` over decoded blocks ([unit-testing.md](./unit-testing.md))
3. **Engine test** with `substreams run` / `--test-file` ([integration-testing.md](./integration-testing.md))

## See also

- [Unit testing](./unit-testing.md)
- [Integration testing](./integration-testing.md)
- [firehose-core](https://github.com/streamingfast/firehose-core)
- [Official testing docs](https://docs.substreams.dev/reference-material/development-tools/testing)
