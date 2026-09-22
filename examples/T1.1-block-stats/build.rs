fn main() {
    buffa_build::Config::new()
        .files(&["proto/stats.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
