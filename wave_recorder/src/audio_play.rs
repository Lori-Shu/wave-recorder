use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, RwLock, atomic::AtomicBool},
    thread::JoinHandle,
};

use anyhow::Context;
use cpal::{
    OutputCallbackInfo, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender};
use my_audio_codec::{AudioCodecResult, codec::TinyDecoder};
use tracing::{info, warn};

use crate::{RECORD_CHANNELS, RECORD_SAMPLE_RATE};

pub struct AudioPlayer {
    output_stream: Arc<Stream>,
    decoder: Arc<RwLock<TinyDecoder>>,
    decode_thread: Option<JoinHandle<AudioCodecResult<()>>>,
    decoding_flag: Arc<AtomicBool>,
    frame_sender: Sender<Vec<f32>>,
    decoding_cond_var: Arc<Condvar>,
    frame_end_flag: Arc<AtomicBool>,
}
impl AudioPlayer {
    pub fn new() -> AudioCodecResult<Self> {
        let decoder = Arc::new(RwLock::new(TinyDecoder::new()?));
        let (frame_sender, recv) = crossbeam_channel::unbounded();
        let decoding_flag = Arc::new(AtomicBool::new(false));
        let decoding_cond_var = Arc::new(Condvar::new());
        let (output_stream, frame_end_flag) =
            Self::rebuild_stream(recv, decoding_cond_var.clone())?;
        Ok(Self {
            output_stream: Arc::new(output_stream),
            decoder,
            decoding_flag,
            decode_thread: None,
            frame_sender,
            decoding_cond_var,
            frame_end_flag,
        })
    }
    fn rebuild_stream(
        recv: Receiver<Vec<f32>>,
        decoding_cond_var: Arc<Condvar>,
    ) -> AudioCodecResult<(Stream, Arc<AtomicBool>)> {
        let default_host = cpal::default_host();
        let device = default_host
            .default_output_device()
            .context("open output stream err")?;

        let config = StreamConfig {
            channels: RECORD_CHANNELS as u16,
            sample_rate: RECORD_SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Default,
        };

        let (audio_output_stream_callback, frame_end_flag) =
            AudioOutputStreamCallback::new(recv, decoding_cond_var);
        let output_stream = device.build_output_stream(
            &config,
            audio_output_stream_callback.into_closure(),
            |e| warn!("{}", e),
            None,
        )?;
        Ok((output_stream, frame_end_flag))
    }
    pub fn pause(&self) -> AudioCodecResult<()> {
        self.output_stream.pause()?;
        Ok(())
    }
    pub fn play(&mut self) -> AudioCodecResult<()> {
        self.output_stream.play()?;
        Ok(())
    }
    pub fn is_end(&mut self) -> bool {
        self.frame_end_flag
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn reset_player(
        &mut self,
        file_path: PathBuf,
        sample_sender: Sender<f32>,
    ) -> AudioCodecResult<()> {
        if let Some(th) = self.decode_thread.take() {
            self.decoding_flag
                .store(false, std::sync::atomic::Ordering::Relaxed);
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
        self.decoding_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let is_decoding = self.decoding_flag.clone();
        let frame_sender = self.frame_sender.clone();
        let condvar = self.decoding_cond_var.clone();
        let stream = self.output_stream.clone();
        let decode_fn = DecodeFn::new(
            decoder,
            frame_sender,
            is_decoding,
            condvar,
            stream,
            sample_sender,
        );
        self.decode_thread = Some(std::thread::spawn(decode_fn.into_closure()));
        Ok(())
    }
}

struct AudioOutputStreamCallback {
    frame_recv: Receiver<Vec<f32>>,
    decoding_cond_var: Arc<Condvar>,
    audio_buffer: Arc<Mutex<VecDeque<f32>>>,
    frame_end_flag: Arc<AtomicBool>,
}
impl AudioOutputStreamCallback {
    pub fn new(
        frame_recv: Receiver<Vec<f32>>,
        decoding_cond_var: Arc<Condvar>,
    ) -> (Self, Arc<AtomicBool>) {
        let audio_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let frame_end_flag = Arc::new(AtomicBool::new(false));
        (
            Self {
                frame_recv,
                decoding_cond_var,
                audio_buffer,
                frame_end_flag: frame_end_flag.clone(),
            },
            frame_end_flag,
        )
    }
    fn into_closure(self) -> impl FnMut(&mut [f32], &OutputCallbackInfo) + Send + 'static {
        move |buf, _| {
            if let Ok(mut buffer_lock) = self.audio_buffer.lock() {
                if buffer_lock.len() < buf.len() {
                    self.decoding_cond_var.notify_one();
                    if let Ok(item) = self.frame_recv.try_recv() {
                        buffer_lock.extend(item);
                        let buf_part = buffer_lock.drain(0..buf.len()).collect::<Vec<f32>>();
                        buf.copy_from_slice(&buf_part);
                    } else {
                        self.frame_end_flag
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                } else {
                    let buf_part = buffer_lock.drain(0..buf.len()).collect::<Vec<f32>>();
                    buf.copy_from_slice(&buf_part);
                }
            }
        }
    }
}
struct DecodeFn {
    decoder: Arc<RwLock<TinyDecoder>>,
    frame_sender: Sender<Vec<f32>>,
    decoding_flag: Arc<AtomicBool>,
    decoding_cond_var: Arc<Condvar>,
    decode_buffer: VecDeque<Vec<f32>>,
    stream: Arc<Stream>,
    sample_sender: Sender<f32>,
}
impl DecodeFn {
    pub fn new(
        decoder: Arc<RwLock<TinyDecoder>>,
        frame_sender: Sender<Vec<f32>>,
        decoding_flag: Arc<AtomicBool>,
        decoding_cond_var: Arc<Condvar>,
        stream: Arc<Stream>,
        sample_sender: Sender<f32>,
    ) -> Self {
        let decode_buffer = VecDeque::new();
        Self {
            decoder,
            frame_sender,
            decoding_flag,
            decoding_cond_var,
            decode_buffer,
            stream,
            sample_sender,
        }
    }
    fn into_closure(mut self) -> impl FnOnce() -> AudioCodecResult<()> + Send + 'static {
        move || {
            let mut decoder = self
                .decoder
                .write()
                .map_err(|_| anyhow::Error::msg("lock decoder err"))?;
            let flag_lock = Mutex::new(());
            let mut counter = 0;
            let mut chunk_max = 0.0_f32;
            loop {
                if self.decode_buffer.len() < 32 {
                    if let Ok(pop_frame) = decoder.pop_frame() {
                        for item in &pop_frame.0 {
                            if counter == 0 {
                                self.sample_sender.send(chunk_max)?;
                                chunk_max = 0.0;
                            }
                            chunk_max = chunk_max.max((*item).abs());
                            counter += 1;
                            counter %= 480;
                        }
                        self.decode_buffer.push_back(pop_frame.0);
                    } else {
                        self.stream.pause()?;
                        return Ok(());
                    }
                } else {
                    self.frame_sender
                        .send(self.decode_buffer.drain(0..32).flatten().collect())?;
                    let mutex_guard = flag_lock
                        .lock()
                        .map_err(|_| anyhow::Error::msg("flag lock err"))?;
                    let _guard = self
                        .decoding_cond_var
                        .wait(mutex_guard)
                        .map_err(|_| anyhow::Error::msg("wait decoding cond var"))?;
                }
                if !self
                    .decoding_flag
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
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
