fn main() {
    buffa_build::Config::new()
        .files(&["proto/marinade/deposit/v1/deposits.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
