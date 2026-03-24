// use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
// use tracing::{Level, info};
// use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
// use wave_recorder::WaveRecorder;
// use wgpu::{Backends, InstanceDescriptor};
// #[cfg(target_os = "windows")]
// fn main() {
//     unsafe {
//         std::env::set_var("RUST_BACKTRACE", "1");
//     }
//     let targets_filter = tracing_subscriber::filter::Targets::default()
//         .with_default(Level::WARN)
//         .with_target("my_audio_codec", Level::INFO)
//         .with_target("wave_recorder", Level::INFO);

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
//     let options = eframe::NativeOptions {
//         renderer: eframe::Renderer::Wgpu,
//         wgpu_options: WgpuConfiguration {
//             wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
//                 instance_descriptor: InstanceDescriptor {
//                     backends: Backends::VULKAN,
//                     ..Default::default()
//                 },
//                 ..Default::default()
//             }),
//             ..Default::default()
//         },
//         ..Default::default()
//     };
//     eframe::run_native(
//         "wave-recorder",
//         options,
//         Box::new(|cc| {
//             let recorder = match WaveRecorder::new(cc) {
//                 Ok(recorder) => recorder,
//                 Err(e) => panic!("new WaveRecorder err{}", e),
//             };
//             Ok(Box::new(recorder))
//         }),
//     )
//     .unwrap();
// }
fn main() {}
