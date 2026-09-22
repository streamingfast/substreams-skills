fn main() {
    buffa_build::Config::new()
        .files(&["proto/nft_mints.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
