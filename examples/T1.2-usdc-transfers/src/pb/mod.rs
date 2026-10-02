#[allow(clippy::all)]
pub mod usdc {
    pub mod transfers {
        pub mod v1 {
            include!("usdc.transfers.v1.mod.rs");
        }
    }
}
