#[allow(clippy::all)]
pub mod eth {
    pub mod stats {
        pub mod v1 {
            include!("eth.stats.v1.mod.rs");
        }
    }
}
