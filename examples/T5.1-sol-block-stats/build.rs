fn main() {
    buffa_build::Config::new()
        .files(&["proto/sol/v1/sol.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
