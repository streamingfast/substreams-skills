#[allow(clippy::all)]
pub mod dex {
    pub mod volume {
        pub mod v1 {
            include!("dex.volume.v1.mod.rs");
        }
    }
}
#[allow(clippy::all)]
pub mod sf {
    pub mod substreams {
        pub mod sink {
            pub mod entity {
                pub mod v1 {
                    include!("sf.substreams.sink.entity.v1.mod.rs");
                }
            }
        }
    }
}
