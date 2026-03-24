use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, RwLock, atomic::AtomicBool},
    thread::JoinHandle,
};

use cpal::{
    OutputCallbackInfo, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender};
use my_audio_codec::{AudioCodecResult, codec::TinyDecoder};
use tracing::{info, warn};

use crate::{RECORD_CHANNELS, RECORD_SAMPLE_RATE};

pub struct AudioPlayer {
    output_stream: Stream,
    decoder: Arc<RwLock<TinyDecoder>>,
    decode_thread: Option<JoinHandle<AudioCodecResult<()>>>,
    is_decoding: Arc<AtomicBool>,
    sender: Sender<Vec<f32>>,
    decoding_cond_var: Arc<Condvar>,
}
impl AudioPlayer {
    pub fn new() -> AudioCodecResult<Self> {
        let decoder = Arc::new(RwLock::new(TinyDecoder::new()?));
        let (sender, recv) = crossbeam_channel::unbounded();
        let is_decoding = Arc::new(AtomicBool::new(false));
        let default_host = cpal::default_host();
        let device = default_host
            .default_output_device()
            .ok_or(anyhow::Error::msg("open output stream err"))?;
        let config = StreamConfig {
            channels: RECORD_CHANNELS as u16,
            sample_rate: RECORD_SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Default,
        };
        let decoding_cond_var = Arc::new(Condvar::new());
        let audio_output_stream_callback =
            AudioOutputStreamCallback::new(recv, decoding_cond_var.clone());
        let output_stream = device.build_output_stream(
            &config,
            audio_output_stream_callback.into_closure(),
            |e| warn!("{}", e),
            None,
        )?;
        Ok(Self {
            output_stream,
            decoder,
            is_decoding,
            decode_thread: None,
            sender,
            decoding_cond_var,
        })
    }
    pub fn pause(&self) -> AudioCodecResult<()> {
        self.output_stream.pause()?;
        Ok(())
    }
    pub fn play(&mut self) -> AudioCodecResult<()> {
        self.output_stream.play()?;
        Ok(())
    }
    pub fn reset_player(&mut self, file_path: PathBuf) -> AudioCodecResult<()> {
        if let Some(th) = self.decode_thread.take() {
            self.is_decoding
                .store(false, std::sync::atomic::Ordering::Release);
            self.decoding_cond_var.notify_one();
            th.join().map_err(|_| anyhow::Error::msg("join th err"))??;
            info!("decode thread exit successfully");
        }
        {
            let mut decoder = self
                .decoder
                .write()
                .map_err(|_| anyhow::Error::msg("lock decoder err"))?;
            decoder.reset_input_file(file_path)?;
            decoder.read_file_header()?;
        }
        self.output_stream.pause()?;

        let decoder = self.decoder.clone();
        self.is_decoding
            .store(true, std::sync::atomic::Ordering::Release);
        let is_decoding = self.is_decoding.clone();
        let sender = self.sender.clone();
        let decode_fn = DecodeFn::new(decoder, sender, is_decoding, self.decoding_cond_var.clone());
        self.decode_thread = Some(std::thread::spawn(decode_fn.into_closure()));
        Ok(())
    }
}

struct AudioOutputStreamCallback {
    recv: Receiver<Vec<f32>>,
    decoding_cond_var: Arc<Condvar>,
    audio_buffer: Arc<Mutex<VecDeque<f32>>>,
}
impl AudioOutputStreamCallback {
    pub fn new(recv: Receiver<Vec<f32>>, decoding_cond_var: Arc<Condvar>) -> Self {
        let audio_buffer = Arc::new(Mutex::new(VecDeque::new()));
        Self {
            recv,
            decoding_cond_var,
            audio_buffer,
        }
    }
    fn into_closure(self) -> impl FnMut(&mut [f32], &OutputCallbackInfo) + Send + 'static {
        move |buf, _| {
            if let Ok(mut buffer_lock) = self.audio_buffer.lock() {
                if buffer_lock.len() < buf.len() {
                    self.decoding_cond_var.notify_one();
                    if let Ok(item) = self.recv.try_recv() {
                        buffer_lock.extend(item);
                        let buf_slice = buffer_lock
                            .drain(0..buf.len())
                            .map(|i| (i * 10.0).clamp(-1.0, 1.0))
                            .collect::<Vec<f32>>();
                        buf.copy_from_slice(&buf_slice);
                    }
                } else {
                    let buf_slice = buffer_lock
                        .drain(0..buf.len())
                        .map(|i| (i * 10.0).clamp(-1.0, 1.0))
                        .collect::<Vec<f32>>();
                    buf.copy_from_slice(&buf_slice);
                }
            }
        }
    }
}
struct DecodeFn {
    decoder: Arc<RwLock<TinyDecoder>>,
    sender: Sender<Vec<f32>>,
    is_decoding: Arc<AtomicBool>,
    decoding_cond_var: Arc<Condvar>,
    decode_buffer: VecDeque<Vec<f32>>,
}
impl DecodeFn {
    pub fn new(
        decoder: Arc<RwLock<TinyDecoder>>,
        sender: Sender<Vec<f32>>,
        is_decoding: Arc<AtomicBool>,
        decoding_cond_var: Arc<Condvar>,
    ) -> Self {
        let decode_buffer = VecDeque::new();
        Self {
            decoder,
            sender,
            is_decoding,
            decoding_cond_var,
            decode_buffer,
        }
    }
    fn into_closure(mut self) -> impl FnOnce() -> AudioCodecResult<()> + Send + 'static {
        move || {
            let mut decoder = self
                .decoder
                .write()
                .map_err(|_| anyhow::Error::msg("lock decoder err"))?;
            let flag_lock = Mutex::new(());
            loop {
                if self.decode_buffer.len() < 10 {
                    let pop_frame = decoder.pop_frame()?;
                    self.decode_buffer.push_back(pop_frame.0);
                } else {
                    self.sender
                        .send(self.decode_buffer.drain(0..10).flatten().collect())?;
                    let mutex_guard = flag_lock
                        .lock()
                        .map_err(|_| anyhow::Error::msg("flag lock err"))?;
                    let _guard = self
                        .decoding_cond_var
                        .wait(mutex_guard)
                        .map_err(|_| anyhow::Error::msg("wait decoding cond var"))?;
                }
                if !self.is_decoding.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
            }
            Ok(())
        }
    }
}
// struct PlayFn {
//     player: Arc<Player>,
//     thread_is_playing: Arc<AtomicBool>,
//     decoder: Arc<RwLock<TinyDecoder>>,
//     file_path: PathBuf,
// }
// impl PlayFn {
//     pub fn new(
//         player: Arc<Player>,
//         thread_is_playing: Arc<AtomicBool>,
//         decoder: Arc<RwLock<TinyDecoder>>,
//         file_path: PathBuf,
//     ) -> Self {
//         Self {
//             player,
//             thread_is_playing,
//             decoder,
//             file_path,
//         }
//     }
//     fn into_closure(self) -> impl FnOnce() -> AudioCodecResult<()> + Send + 'static {
//         move || {
//             let mut decoder = self
//                 .decoder
//                 .write()
//                 .map_err(|_e| anyhow::Error::msg("decoder write lock err"))?;

//             decoder.reset_input_file(self.file_path)?;
//             decoder.read_file_header()?;
//             loop {
//                 if !self.thread_is_playing.load(std::sync::atomic::Ordering::Acquire) {
//                     break;
//                 }
//                 if self.player.len() < 10 {
//                     if let Ok((frame, header)) = decoder.pop_frame() {
//                         if let Some(channels) = NonZero::new(header.channels() as u16)
//                             && let Some(sample_rate) = NonZero::new(header.sample_rate())
//                         {
//                             // info!("decoded debug sample:{},player len:{}",frames[0],self.player.len());
//                             let samples_buffer = SamplesBuffer::new(channels, sample_rate, frame);

//                             self.player.append(samples_buffer);
//                         }
//                     }else{
//                         warn!("pop frames err");
//                     }
//                 }else{
//                     std::thread::sleep(Duration::from_millis(1));
//                 }
//             }
//             Ok(())
//         }
//     }
// }
