use substreams_ethereum::Abigen;

fn main() -> Result<(), anyhow::Error> {
    buffa_build::Config::new()
        .files(&["proto/dex_volume.proto", "proto/entity.proto"])
        .includes(&["proto/"])
        .preserve_unknown_fields(false)
        .compile()
        .expect("compiling protos");

    for name in ["erc20", "uniswap_v2_pair", "uniswap_v3_pool"] {
        Abigen::new(name, &format!("abi/{}.json", name))?
            .generate()?
            .write_to_file(&format!("src/abi/{}.rs", name))?;
    }
    Ok(())
}
