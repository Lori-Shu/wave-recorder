#![deny(unused)]
#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
pub mod codec;
pub type AudioCodecResult<T> = anyhow::Result<T>;
