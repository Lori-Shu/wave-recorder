// #![cfg_attr(
//     all(target_os = "windows", not(debug_assertions)),
//     windows_subsystem = "windows"
// )]
// #![deny(unused)]

// use my_audio_codec::codec::{TinyDecoder, TinyEncoder};
// use tracing::{Level, info, warn};
// use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// fn main() {
//     unsafe {
//         std::env::set_var("RUST_BACKTRACE", "1");
//     }
//     let targets_filter = tracing_subscriber::filter::Targets::default()
//         .with_default(Level::WARN)
//         .with_target("my_audio_codec", Level::INFO);

//     let subscriber = tracing_subscriber::registry::Registry::default()
//         .with(
//             tracing_subscriber::fmt::layer()
//                 .with_thread_ids(true)
//                 .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339()),
//         )
//         .with(targets_filter);
//     subscriber.init();
//     let span = tracing::span!(Level::INFO, "main");
//     let _main_entered = span.enter();
//     info!("enter main span");
//     // _test_encode();
//     _test_decode();
// }
// fn _test_encode() {
//     if let Ok(mut enc) = TinyEncoder::new() {
//         if let Err(e) = enc.encode() {
//             warn!("{}", e.backtrace());
//         }
//     }
// }
// fn _test_decode() {
//     if let Ok(mut decoder) = TinyDecoder::new() {
//         if let Err(e) = decoder.decode() {
//             warn!("{}", e);
//             warn!("{}", e.backtrace());
//         }
//     }
// }
fn main() {}
