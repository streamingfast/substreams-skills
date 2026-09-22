fn main() {
    buffa_build::Config::new()
        .files(&["proto/univ2_swaps.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .unwrap();
}
