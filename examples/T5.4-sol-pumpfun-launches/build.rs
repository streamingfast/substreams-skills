fn main() {
    buffa_build::Config::new()
        .files(&["proto/pumpfun/v1/launches.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
