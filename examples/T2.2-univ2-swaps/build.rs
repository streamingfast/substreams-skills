use substreams_ethereum::Abigen;

fn main() -> Result<(), anyhow::Error> {

    for name in ["erc20", "uniswap_v2_pair"] {
        Abigen::new(name, &format!("abi/{}.json", name))?
            .generate()?
            .write_to_file(&format!("src/abi/{}.rs", name))?;
    }
    Ok(())
}
