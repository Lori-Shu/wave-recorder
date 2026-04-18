#![deny(unused)]
#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, RwLock, atomic::AtomicBool},
    thread::JoinHandle,
    time::Duration,
};

use cpal::{
    InputCallbackInfo, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender};
use eframe::{
    App, CreationContext,
    egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew},
    wgpu::{
        BackendOptions, Backends, DeviceDescriptor, InstanceDescriptor, InstanceFlags,
        MemoryBudgetThresholds,
    },
};
use egui::{
    AtomExt, Button, Color32, ComboBox, ImageSource, Layout, Pos2, Rect, RichText, ScrollArea,
    SizeHint, TextureOptions, Ui, Vec2,
    emath::OrderedFloat,
    include_image,
    load::{SizedTexture, TexturePoll},
};
use egui_plot::{Line, Plot, PlotPoints};
use my_audio_codec::{AudioCodecResult, codec::TinyEncoder};
use tracing::{Level, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    audio_play::AudioPlayer,
    effect_filter::{EffectFilter, SelectedEffect},
};
mod audio_play;
mod effect_filter;

const PLAY_IMG: ImageSource = include_image!("../resources/play.png");
const PAUSE_IMG: ImageSource = include_image!("../resources/pause.png");
const STOP_IMG: ImageSource = include_image!("../resources/circle-stop.png");
const BACKGROUND_IMG: ImageSource = include_image!("../resources/background.png");
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let targets_filter = tracing_subscriber::filter::Targets::default()
        .with_default(Level::WARN)
        .with_target("my_audio_codec", Level::INFO)
        .with_target("wave_recorder", Level::INFO);

    let subscriber = tracing_subscriber::registry::Registry::default()
        .with(
            tracing_subscriber::fmt::layer()
                .with_thread_ids(true)
                .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339()),
        )
        .with(targets_filter);
    subscriber.init();
    let span = tracing::span!(Level::INFO, "main");
    let _main_entered = span.enter();
    info!("enter main span");
    let options = eframe::NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: WgpuConfiguration {
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: InstanceDescriptor {
                    backends: Backends::VULKAN,
                    flags: InstanceFlags::default(),
                    memory_budget_thresholds: MemoryBudgetThresholds::default(),
                    backend_options: BackendOptions::default(),
                    display: None,
                },
                display_handle: None,
                power_preference: eframe::wgpu::PowerPreference::None,
                native_adapter_selector: None,
                device_descriptor: Arc::new(|_adapter| DeviceDescriptor::default()),
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "wave-recorder",
        options,
        Box::new(|cc| {
            let recorder = match WaveRecorder::new(cc) {
                Ok(recorder) => recorder,
                Err(e) => panic!("new WaveRecorder err{}", e),
            };
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(recorder))
        }),
    )
    .unwrap();
}
enum RouterPage {
    Main,
    List,
    Message(String),
}
const RECORD_SAMPLE_RATE: u32 = 48000;
const RECORD_CHANNELS: u8 = 2;
const APP_DATA_FOLDER: &str = "/storage/emulated/0/Android/data/org.my_audio_codec/";
pub struct WaveRecorder {
    microphone_manager: MicroPhoneManager,
    my_encoder: Arc<RwLock<TinyEncoder>>,
    router_page: RouterPage,
    is_recording: bool,
    is_recording_started: bool,
    is_playing: bool,
    playing_record_path: Option<PathBuf>,
    available_records: Vec<PathBuf>,
    audio_player: AudioPlayer,
    is_encoding: Arc<AtomicBool>,
    encode_thread: Option<JoinHandle<AudioCodecResult<()>>>,
    background_tex_poll: Option<SizedTexture>,
    selected_effect: Arc<RwLock<SelectedEffect>>,
    line_points_queue: VecDeque<f32>,
    point_sample_channel: (Sender<f32>, Receiver<f32>),
}
impl WaveRecorder {
    pub fn new(_e_context: &CreationContext) -> AudioCodecResult<Self> {
        let is_recording = false;
        let microphone_manager = MicroPhoneManager::new()?;
        let my_encoder = Arc::new(RwLock::new(TinyEncoder::new(
            PathBuf::from(APP_DATA_FOLDER),
            RECORD_SAMPLE_RATE,
            RECORD_CHANNELS,
        )?));
        let audio_player = AudioPlayer::new()?;
        let selected_effect = Arc::new(RwLock::new(SelectedEffect::None));
        let line_points_queue = VecDeque::new();
        let point_sample_channel = crossbeam_channel::unbounded();
        Ok(Self {
            microphone_manager,
            my_encoder,
            router_page: RouterPage::Main,
            is_recording,
            is_recording_started: false,
            is_playing: false,
            available_records: vec![],
            audio_player,
            encode_thread: None,
            is_encoding: Arc::new(AtomicBool::new(false)),
            playing_record_path: None,
            background_tex_poll: None,
            selected_effect,
            line_points_queue,
            point_sample_channel,
        })
    }
    fn paint_main_page(&mut self, ui: &mut Ui) {
        let _ = ui.button("wave-recorder: a simple voice recorder");
        self.paint_wave_line(ui);
        if ui.button("record list").clicked() {
            if self.scan_available_records().is_ok() {
                self.router_page = RouterPage::List;
            }
        }
        ui.add_space(ui.content_rect().height() / 10.0);
        if let Ok(mut selected_effect) = self.selected_effect.write() {
            ComboBox::new(
                "effect combobox",
                RichText::new("select audio effect").strong(),
            )
            .selected_text(format!("selected:{}", *selected_effect))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut *selected_effect, SelectedEffect::None, "no effect");
                ui.selectable_value(&mut *selected_effect, SelectedEffect::Wave, "wave effect");
                ui.selectable_value(
                    &mut *selected_effect,
                    SelectedEffect::CicadaChirping,
                    "cicada chirping",
                );
                ui.selectable_value(&mut *selected_effect, SelectedEffect::Rain, "rain effect");
                ui.selectable_value(&mut *selected_effect, SelectedEffect::Piano, "piano effect");
            });
        }

        ui.add_space(ui.content_rect().height() / 10.0);
        if self.is_recording {
            ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                let pause_btn = Button::new(PAUSE_IMG.atom_size(Vec2::new(
                    ui.content_rect().height() / 10.0,
                    ui.content_rect().height() / 10.0,
                )));
                let pause_response = ui.add(pause_btn);
                if pause_response.clicked() {
                    self.pause_recording();
                }
            });
        } else {
            ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                let continue_btn = Button::new(PLAY_IMG.atom_size(Vec2::new(
                    ui.content_rect().height() / 10.0,
                    ui.content_rect().height() / 10.0,
                )));
                let continue_response = ui.add(continue_btn);
                if continue_response.clicked() {
                    if !self.is_recording_started {
                        if let Ok(mut encoder) = self.my_encoder.write() {
                            if let Err(e) = encoder.reset_encoder() {
                                self.router_page = RouterPage::Message(format!("{}", e));
                            }
                        }
                        self.is_recording_started = true;
                    }
                    if self.start_encoding().is_ok() {
                        info!("start encoding ok");
                    }
                }
                ui.add_space(ui.content_rect().height() / 10.0);
                let finish_btn = Button::new(STOP_IMG.atom_size(Vec2::new(
                    ui.content_rect().height() / 10.0,
                    ui.content_rect().height() / 10.0,
                )));
                let finish_response = ui.add(finish_btn);
                if finish_response.clicked() {
                    if self.is_recording_started {
                        if let Ok(mut encoder) = self.my_encoder.write() {
                            if let Err(e) = encoder.save_file() {
                                self.router_page =
                                    RouterPage::Message(format!("end_encode err{}", e));
                            } else {
                                self.router_page =
                                    RouterPage::Message("record is finished and saved".to_string());
                                self.is_recording_started = false;
                            }
                        }
                    } else {
                        self.router_page =
                            RouterPage::Message("recording has not started".to_string());
                    }
                }
            });
        }
    }
    fn paint_list_page(&mut self, ui: &mut Ui) {
        if ui.button("back to main").clicked() {
            self.router_page = RouterPage::Main;
        }
        if let Some(record_file_path) = &self.playing_record_path {
            if let Some(path_str) = record_file_path.file_name() {
                if let Some(file_name) = path_str.to_str() {
                    ui.add(Button::new(
                        RichText::new(format!("playing record:{}", file_name)).strong(),
                    ));
                }
            }
        } else {
            ui.add(Button::new(RichText::new("playing record: None").strong()));
        }
        ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
            if self.is_playing {
                if ui
                    .button(PAUSE_IMG.atom_size(Vec2::new(
                        ui.content_rect().height() / 10.0,
                        ui.content_rect().height() / 10.0,
                    )))
                    .clicked()
                {
                    self.is_playing = false;
                    if self.audio_player.pause().is_ok() {
                        info!("play paused");
                    }
                }
            } else {
                if ui
                    .button(PLAY_IMG.atom_size(Vec2::new(
                        ui.content_rect().height() / 10.0,
                        ui.content_rect().height() / 10.0,
                    )))
                    .clicked()
                {
                    self.is_playing = true;
                    if self.audio_player.play().is_ok() {
                        info!("play continued");
                    }
                }
            }
        });
        ScrollArea::vertical()
            .min_scrolled_height(ui.content_rect().height() / 6.0)
            .show(ui, |ui| {
                ui.columns(1, |ui| {
                    for path in &self.available_records {
                        if let Some(f_name) = path.file_name() {
                            if let Some(file_name) = f_name.to_str() {
                                if ui[0]
                                    .add(Button::new(
                                        RichText::new(format!("{}", file_name)).atom_size(
                                            Vec2::new(
                                                ui[0].content_rect().height() / 6.0,
                                                ui[0].content_rect().height() / 20.0,
                                            ),
                                        ),
                                    ))
                                    .clicked()
                                {
                                    self.playing_record_path = Some(path.clone());
                                    if let Err(e) = self.audio_player.reset_player(path.clone()) {
                                        self.router_page =
                                            RouterPage::Message(format!("reset player err:{}", e));
                                    }
                                    self.is_playing = false;
                                }
                                ui[0].add_space(ui[0].content_rect().height() / 20.0);
                            }
                        }
                    }
                });
            });
    }
    fn paint_msg_page(&mut self, msg: String, ui: &mut Ui) {
        ui.label(msg);
        if ui.button("return to main").clicked() {
            self.router_page = RouterPage::Main;
        }
    }
    fn scan_available_records(&mut self) -> AudioCodecResult<()> {
        self.available_records.clear();
        let path = PathBuf::from(APP_DATA_FOLDER);
        let mut read_dir = path.read_dir()?;
        loop {
            if let Some(Ok(item)) = read_dir.next() {
                if item
                    .file_name()
                    .to_str()
                    .ok_or(anyhow::Error::msg("file name to str err"))?
                    .ends_with(".gla")
                {
                    self.available_records.push(item.path());
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    fn pause_recording(&mut self) {
        self.is_recording = false;
        self.microphone_manager.end_input();

        self.is_encoding
            .store(false, std::sync::atomic::Ordering::Release);
        if let Some(handle) = self.encode_thread.take() {
            match handle.join() {
                Ok(r) => {
                    if let Err(e) = r {
                        warn!("{}", e);
                    }
                }
                Err(_e) => {
                    self.router_page = RouterPage::Message("join encode thread err".to_string());
                }
            }
        }
        info!("after join encode thread");
    }
    fn start_encoding(&mut self) -> AudioCodecResult<()> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        self.microphone_manager.open_audio_input(sender)?;
        self.is_recording = true;
        self.is_encoding
            .store(true, std::sync::atomic::Ordering::Release);
        let selected_effect = self.selected_effect.clone();
        let effect_filter = EffectFilter::new()?;
        let encode_fn = EncodeFn::new(
            self.my_encoder.clone(),
            receiver,
            self.is_encoding.clone(),
            selected_effect,
            effect_filter,
            self.point_sample_channel.0.clone(),
        );
        self.encode_thread = Some(std::thread::spawn(encode_fn.into_closure()));
        Ok(())
    }
    fn paint_background(&mut self, ui: &mut Ui) -> AudioCodecResult<()> {
        if let Some(texture) = &self.background_tex_poll {
            ui.painter().image(
                texture.id,
                ui.content_rect(),
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            let texture_poll = BACKGROUND_IMG.load(
                ui.ctx(),
                TextureOptions::LINEAR,
                SizeHint::Scale(OrderedFloat(1.0)),
            )?;
            if let TexturePoll::Ready { texture } = texture_poll {
                self.background_tex_poll = Some(texture);
            }
        }

        Ok(())
    }
    fn paint_wave_line(&mut self, ui: &mut Ui) {
        if self.is_encoding.load(std::sync::atomic::Ordering::Acquire) {
            if let Ok(sample) = self.point_sample_channel.1.try_recv() {
                self.line_points_queue.push_back(sample);
            }
        }
        let plot = Plot::new("wave line plot")
            .height(ui.content_rect().height() / 4.0)
            .allow_axis_zoom_drag(false)
            .allow_boxed_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false);
        if self.line_points_queue.len() > 100 {
            self.line_points_queue.pop_front();
        }
        let points = self
            .line_points_queue
            .iter()
            .enumerate()
            .map(|(idx, item)| [idx as f64, (*item) as f64])
            .collect::<PlotPoints>();
        let line = Line::new("wave line", points);
        plot.show(ui, |ui| {
            ui.line(line);
        });
    }
}
impl App for WaveRecorder {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Err(e) = self.paint_background(ui) {
                warn!("paint background err:{}", e);
            }
            match &self.router_page {
                RouterPage::Main => self.paint_main_page(ui),
                RouterPage::List => self.paint_list_page(ui),
                RouterPage::Message(msg) => self.paint_msg_page(msg.clone(), ui),
            }
            ui.request_repaint_after(Duration::from_millis(1000 / 30));
        });
    }
}
struct RecordFn {
    sender: Sender<Vec<f32>>,
}
impl RecordFn {
    pub fn new(sender: Sender<Vec<f32>>) -> Self {
        Self { sender }
    }
    fn into_closure(self) -> impl FnMut(&[f32], &InputCallbackInfo) + Send + 'static {
        move |audio_data, _info| {
            if let Err(e) = self.sender.send(audio_data.to_vec()) {
                warn!("{}", e);
            }
        }
    }
}
struct EncodeFn {
    encoder: Arc<RwLock<TinyEncoder>>,
    sample_chunk_recv: Receiver<Vec<f32>>,
    is_encoding: Arc<AtomicBool>,
    selected_effect: Arc<RwLock<SelectedEffect>>,
    effect_filter: EffectFilter,
    paint_sample_sender: Sender<f32>,
}
impl EncodeFn {
    pub fn new(
        encoder: Arc<RwLock<TinyEncoder>>,
        sample_chunk_recv: Receiver<Vec<f32>>,
        is_encoding: Arc<AtomicBool>,
        selected_effect: Arc<RwLock<SelectedEffect>>,
        effect_filter: EffectFilter,
        paint_sample_sender: Sender<f32>,
    ) -> Self {
        Self {
            encoder,
            sample_chunk_recv,
            is_encoding,
            selected_effect,
            effect_filter,
            paint_sample_sender,
        }
    }
    fn into_closure(mut self) -> impl FnOnce() -> AudioCodecResult<()> + Send + 'static {
        move || {
            let mut encoder = self
                .encoder
                .write()
                .map_err(|_e| anyhow::Error::msg("encoder write lock err"))?;
            let mut counter = 0;
            loop {
                if let Ok(mut samples) = self.sample_chunk_recv.recv() {
                    for item in &mut samples {
                        *item = (*item * 60.0).clamp(-1.0, 1.0);
                        if counter == 0 {
                            self.paint_sample_sender.send(*item)?;
                        }
                        counter += 1;
                        counter %= 480;
                    }
                    let effect = self
                        .selected_effect
                        .read()
                        .map_err(|_e| anyhow::Error::msg("read lock selected effect err"))?;
                    let samples = self.effect_filter.filter(samples, effect.clone())?;
                    if encoder.encode(samples).is_err() {
                        warn!("encode sample err");
                    }
                } else {
                    if !self.is_encoding.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }
                }
            }
            Ok(())
        }
    }
}

struct MicroPhoneManager {
    #[allow(unused)]
    record_stream: Option<Stream>,
}
impl MicroPhoneManager {
    pub fn new() -> AudioCodecResult<Self> {
        Ok(Self {
            record_stream: None,
        })
    }
    fn open_audio_input(&mut self, sender: Sender<Vec<f32>>) -> AudioCodecResult<()> {
        let record_fn = RecordFn::new(sender);
        let default_host = cpal::default_host();
        let default_input_device = default_host
            .default_input_device()
            .ok_or(anyhow::Error::msg("no default audio input device"))?;
        let config = StreamConfig {
            channels: RECORD_CHANNELS as u16,
            sample_rate: RECORD_SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Default,
        };
        let audio_stream = default_input_device.build_input_stream(
            &config,
            record_fn.into_closure(),
            |e| warn!("audio open input err:{}", e),
            None,
        )?;
        audio_stream.play()?;
        self.record_stream = Some(audio_stream);
        Ok(())
    }
    fn end_input(&mut self) {
        if let Some(_stream) = self.record_stream.take() {
            info!("record stream dropped");
        }
    }
}
