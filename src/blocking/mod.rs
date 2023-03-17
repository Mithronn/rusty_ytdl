mod info;

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

pub static TOKIO_RT: Lazy<Runtime> =
    Lazy::new(|| Runtime::new().expect("[ERROR] Unable to start the tokio Runtime"));

#[macro_export]
#[cfg(feature = "blocking")]
macro_rules! block_async {
    (async $future:block) => { $crate::blocking::TOKIO_RT.block_on(async $future) };
    (async move $future:block) => { $crate::blocking::TOKIO_RT.block_on(async move $future) };
    ($future:expr) => {
        $crate::blocking::TOKIO_RT.block_on(async {
            $future.await
        })
    };
}

pub use info::Video;
