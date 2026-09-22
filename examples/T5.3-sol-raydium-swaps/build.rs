fn main() {
    buffa_build::Config::new()
        .files(&["proto/raydium/clmm/v1/swaps.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
