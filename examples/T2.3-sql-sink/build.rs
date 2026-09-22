fn main() {
    buffa_build::Config::new()
        .files(&["proto/usdc_transfers.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
