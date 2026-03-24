use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{BufReader, BufWriter, Read},
    ops::Range,
    path::{Path, PathBuf},
};

use bitstream_io::{BitRead2, BitReader, BitWrite2, BitWriter, LittleEndian};
use hound::{SampleFormat, WavSpec};

use rustdct::{
    DctPlanner,
    mdct::{Mdct, MdctViaDct4},
};
use time::format_description;
use tracing::{info, warn};

use crate::AudioCodecResult;
const DATA_FRAME_SIZE: usize = 1024;
const QUALITY_FACTOR: f32 = 1.4;
const MAX_REMAINDER_LEN: u32 = 8;
pub struct TinyEncoder {
    bands: Vec<Range<usize>>,
    file_bit_writer: Option<BitWriter<BufWriter<File>, LittleEndian>>,
    delta_table: Vec<f32>,
    samples_cache: VecDeque<f32>,
    mdct_via_dct4: MdctViaDct4<f32>,
    scratch_buf: Vec<f32>,
    frames: u64,
    indicies: Vec<Vec<u8>>,
    header_manager: CodecHeaderManager,
    app_folder: PathBuf,
    sample_rate: u32,
    channels: u8,
    last_half: Vec<f32>,
}
impl TinyEncoder {
    pub fn new(app_folder: PathBuf, sample_rate: u32, channels: u8) -> AudioCodecResult<Self> {
        let mut delta_table = vec![0.0_f32; 64];
        let base_d = 4e-7_f32;
        let ratio = 1.24_f32;
        for idx in 0..64 {
            delta_table[idx as usize] = base_d * ratio.powi(idx);
        }
        let mut dct_planner = DctPlanner::<f32>::new();
        let plan_dct4 = dct_planner.plan_dct4(DATA_FRAME_SIZE / 2);
        let mdct_via_dct4 =
            rustdct::mdct::MdctViaDct4::new(plan_dct4, rustdct::mdct::window_fn::vorbis::<f32>);

        let scratch_buf = vec![0.0; DATA_FRAME_SIZE];
        let last_half = vec![0.0_f32; DATA_FRAME_SIZE / 2];
        Ok(Self {
            bands: vec![
                0..2,
                2..4,
                4..8,
                8..16,
                16..32,
                32..64,
                64..128,
                128..256,
                256..512,
            ],
            file_bit_writer: None,
            delta_table,
            samples_cache: VecDeque::new(),
            mdct_via_dct4,
            scratch_buf,
            indicies: vec![],
            frames: 0,
            header_manager: CodecHeaderManager::new(),
            app_folder,
            sample_rate,
            channels,
            last_half,
        })
    }
    pub fn encode(&mut self, chunk_samples: Vec<f32>) -> AudioCodecResult<()> {
        self.samples_cache.extend(chunk_samples);
        loop {
            if let Ok(samples) = self.try_pop_frame() {
                let mdct_output = self.apply_mdct(samples)?;
                let band_energy = self.compute_band_energies(&mdct_output)?;
                let masking_threshold = self.compute_masking_threshold(&band_energy)?;
                let quantized_and_delta_indices =
                    self.quantize(&mdct_output, &masking_threshold)?;
                self.rice_compress(quantized_and_delta_indices.data)?;
                self.frames += 1;
                self.indicies.extend(
                    quantized_and_delta_indices
                        .indices
                        .iter()
                        .map(|i| i.iter().map(|a| (*a) as u8).collect()),
                );
            } else {
                break;
            }
        }
        Ok(())
    }
    fn apply_mdct(&mut self, samples: Vec<f32>) -> AudioCodecResult<Vec<f32>> {
        // let rec = rerun::RecordingStreamBuilder::new("mdct_debug")
        //     .spawn()
        //     .map_err(AudioCodecError::from_err)?;
        // let mut spectrogram: VecDeque<Vec<f32>> = VecDeque::new();
        // const MAX_SPECTROGRAM_LEN: usize = 300;

        let mut mdct_output = vec![0.0; DATA_FRAME_SIZE / 2];

        self.mdct_via_dct4.process_mdct_with_scratch(
            &self.last_half,
            &samples,
            &mut mdct_output,
            &mut self.scratch_buf,
        );

        // rec.set_time_sequence("frame_idx", (offset / DATA_FRAME_SIZE) as i64);

        // let db_output = mdct_output
        //     .iter()
        //     .map(|&x| {
        //         let db = 20.0 * (x.abs() + 1e-6).log10();
        //         ((db + 80.0) / 80.0).clamp(0.0, 1.0)
        //     })
        //     .collect::<Vec<f32>>();
        // if offset % (DATA_FRAME_SIZE * 100) == 0 {
        // info!("Progress: {}/{}", offset, samples.len());
        // let flat = spectrogram.iter().flatten().copied().collect::<Vec<_>>();
        // let array = Array2::from_shape_vec((spectrogram.len(), DATA_FRAME_SIZE / 2), flat)
        //     .map_err(AudioCodecError::from_err)?;
        // rec.log(
        //     "audio/spectrogram",
        //     &Tensor::try_from(array).map_err(AudioCodecError::from_err)?,
        // )
        // .map_err(AudioCodecError::from_err)?;
        // }
        // spectrogram.push_back(db_output);
        // if spectrogram.len() > MAX_SPECTROGRAM_LEN {
        //     spectrogram.pop_front();
        // }
        self.last_half = samples;
        // info!("mdct completed!");
        let scale = (2.0 / DATA_FRAME_SIZE as f32).sqrt();

        for sample in &mut mdct_output {
            (*sample) *= scale;
        }

        Ok(mdct_output)
    }
    fn rice_compress(&mut self, frame_quantized: Vec<i32>) -> AudioCodecResult<()> {
        // info!("into rice_compress");
        for band_range in &self.bands {
            // let mut total_k_bits = 0;
            // let mut total_quotient_bits = 0;
            // let mut total_remainder_bits = 0;
            // let mut total_zeros = 0;
            // let mut total_non_zeros = 0;
            let band_values = &frame_quantized[band_range.clone()];
            let sum_u = band_values
                .iter()
                .map(|&q| ((q << 1) ^ (q >> 31)) as u64)
                .sum::<u64>();
            let mean_u = sum_u as f64 / band_values.len() as f64;

            let k_opt = if mean_u <= 1.0 {
                0
            } else {
                (mean_u.log2().floor() as u32).min(MAX_REMAINDER_LEN)
            };
            let tmp_bit_writer = self
                .file_bit_writer
                .as_mut()
                .ok_or(anyhow::Error::msg("file bit writer err"))?;
            self.header_manager
                .write_remainder_len(tmp_bit_writer, k_opt)?;

            for q in band_values {
                // if *q == 0 {
                //     total_zeros += 1;
                // } else {
                //     total_non_zeros += 1;
                // }
                // let u = ((*q << 1) ^ (*q >> 31)) as u32;
                // let q_bits = (u >> k_opt) + 1; // 商数位 + 1位停止位
                // let r_bits = k_opt; // 余数位
                // total_quotient_bits += q_bits;
                // total_remainder_bits += r_bits;
                let u = ((*q << 1) ^ (*q >> 31)) as u32;

                let quotient = u >> k_opt;
                let remainder = u & ((1 << k_opt) - 1);

                tmp_bit_writer.write_unary1(quotient)?;

                tmp_bit_writer.write(k_opt, remainder)?;
            }
            // info!(
            //     "Band Debug: 0占比: {:.1}%, K={}, 商数总位: {}, 余数总位: {}, 平均每样点位深: {:.2}",
            //     (total_zeros as f32 / (total_zeros + total_non_zeros) as f32) * 100.0,
            //     k_opt,
            //     total_quotient_bits,
            //     total_remainder_bits,
            //     (total_quotient_bits + total_remainder_bits) as f32
            //         / band_values.len() as f32
            // );
        }

        Ok(())
    }
    fn _read_file_to_samples(&self, file_path: &Path) -> AudioCodecResult<Vec<f32>> {
        let mut reader = hound::WavReader::open(file_path)?;
        let wav_spec = reader.spec();
        match wav_spec.sample_format {
            SampleFormat::Int => {
                if wav_spec.bits_per_sample == 16 {
                    Ok(reader
                        .samples::<i16>()
                        .map(|s| {
                            if let Ok(s) = s {
                                s as f32 / (i16::MAX as f32)
                            } else {
                                info!("err !writing 0.0");
                                0.0
                            }
                        })
                        .collect::<Vec<f32>>())
                } else {
                    Err(anyhow::Error::msg(
                        "input sample format is not i16".to_string(),
                    ))
                }
            }
            SampleFormat::Float => Ok(reader.samples::<f32>().collect::<Result<Vec<f32>, _>>()?),
        }
    }
    fn try_pop_frame(&mut self) -> AudioCodecResult<Vec<f32>> {
        let mut frame = vec![];
        if self.samples_cache.len() >= DATA_FRAME_SIZE / 2 {
            frame.extend(self.samples_cache.drain(0..DATA_FRAME_SIZE / 2));
        }
        if frame.is_empty() {
            Err(anyhow::Error::msg(
                "do not have enough samples to make frame",
            ))
        } else {
            Ok(frame)
        }
    }
    fn compute_band_energies(&self, mdct_output: &Vec<f32>) -> AudioCodecResult<Vec<f32>> {
        // info!("into compute_band_energies");
        let mut band_energy = vec![0.0_f32; self.bands.len()];
        for (band_idx, band_range) in self.bands.iter().enumerate() {
            for idx in band_range.clone() {
                let x = mdct_output[idx];
                band_energy[band_idx] += x * x;
            }
        }

        Ok(band_energy)
    }
    fn compute_masking_threshold(&self, band_energy: &Vec<f32>) -> AudioCodecResult<Vec<f32>> {
        // info!("into compute_masking_threshold");
        let mut masking_threshold = vec![0.0_f32; self.bands.len()];

        for (band_idx, _band_range) in self.bands.iter().enumerate() {
            let freq_idx = band_idx as f32 / self.bands.len() as f32;

            let snr_weight = if freq_idx > 0.1 && freq_idx < 0.4 {
                0.01
            } else {
                0.04
            };

            let thresh = band_energy[band_idx] * snr_weight;

            let ath_guard = 1e-8;

            masking_threshold[band_idx] = thresh.max(ath_guard);
        }
        Ok(masking_threshold)
    }
    fn quantize(
        &self,
        mdct_output: &Vec<f32>,
        masking_threshold: &Vec<f32>,
    ) -> AudioCodecResult<QuantizedAndDeltaIndices> {
        // info!("into quantize");
        let mut delta_indices = vec![];
        let mut quantized = vec![0_i32; DATA_FRAME_SIZE / 2];
        let mut single_frame_indices = vec![0_usize; self.bands.len()];
        for (band_idx, band_range) in self.bands.iter().enumerate() {
            let n = band_range.len() as f32;
            let mt = masking_threshold[band_idx];
            let d = (12.0 * mt / n).sqrt() * QUALITY_FACTOR;

            let search_res = self.delta_table.binary_search_by(|a| {
                if let Some(or) = a.partial_cmp(&d) {
                    or
                } else {
                    std::cmp::Ordering::Greater
                }
            });
            let chose_idx = match &search_res {
                Ok(idx) => *idx,
                Err(idx) => {
                    if *idx == 0 {
                        0
                    } else if *idx >= self.delta_table.len() {
                        self.delta_table.len() - 1
                    } else {
                        let left = self.delta_table[*idx - 1];
                        let right = self.delta_table[*idx];
                        if (d - left).abs() < (right - d).abs() {
                            *idx - 1
                        } else {
                            *idx
                        }
                    }
                }
            };
            single_frame_indices[band_idx] = chose_idx;
            let delta = self.delta_table[chose_idx];
            for sample_idx in band_range.clone() {
                let x = mdct_output[sample_idx];
                let q_raw = x.abs() / delta;
                quantized[sample_idx] = q_raw.round() as i32 * x.signum() as i32;
            }
        }
        delta_indices.push(single_frame_indices);

        Ok(QuantizedAndDeltaIndices {
            data: quantized,
            indices: delta_indices,
        })
    }
    fn _compute_skip_table(
        &self,
        band_energies: &[Vec<f32>],
        masking_threshold: &[Vec<f32>],
    ) -> AudioCodecResult<Vec<Vec<bool>>> {
        let mut skip_flags = vec![vec![false; band_energies[0].len()]; band_energies.len()];
        for frame_idx in 0..band_energies.len() {
            for energy_idx in 0..band_energies[frame_idx].len() {
                if band_energies[frame_idx][energy_idx] < masking_threshold[frame_idx][energy_idx] {
                    skip_flags[frame_idx][energy_idx] = true;
                }
            }
        }
        Ok(skip_flags)
    }
    fn _find_band_idx(&self, count: usize) -> usize {
        match count % 512 {
            0..2 => 0,
            2..4 => 1,
            4..8 => 2,
            8..16 => 3,
            16..32 => 4,
            32..64 => 5,
            64..128 => 6,
            128..256 => 7,
            256..512 => 8,
            _ => todo!(),
        }
    }
    pub fn save_file(&mut self) -> AudioCodecResult<()> {
        let now_local = time::OffsetDateTime::now_local()?;
        let formatter = format_description::parse("[year]-[month]-[day] [hour]-[minute]-[second]")?;
        let datetime_str = now_local.format(&formatter)?;
        info!("date time str{}", datetime_str);
        let file_path = self.app_folder.join(datetime_str);
        let file = File::create_new(format!(
            "{}.gla",
            file_path
                .to_str()
                .ok_or(anyhow::Error::msg("file path to str err!"))?
        ))?;
        {
            let mut tmp_file_writer = self
                .file_bit_writer
                .take()
                .ok_or(anyhow::Error::msg("take file writer err"))?;
            tmp_file_writer.byte_align()?;
            tmp_file_writer.flush()?;
        }
        let mut end_file_bit_writer = BitWriter::new(BufWriter::new(file));
        self.header_manager
            .write_frames_len(&mut end_file_bit_writer, self.frames)?;

        self.header_manager
            .write_sample_rate(&mut end_file_bit_writer, self.sample_rate)?;
        self.header_manager
            .write_channels(&mut end_file_bit_writer, self.channels)?;
        self.header_manager
            .write_delta_indices(&mut end_file_bit_writer, &self.indicies)?;
        let tmp_file_path = self.app_folder.join("record_tmp");
        let mut tmp_file = File::open(&tmp_file_path)?;
        let mut buffer = vec![0_u8; 1024];
        loop {
            let read_size = tmp_file.read(&mut buffer)?;
            if read_size == 0 {
                break;
            }
            end_file_bit_writer.write_bytes(&buffer[0..read_size])?;
        }
        end_file_bit_writer.byte_align()?;
        end_file_bit_writer.flush()?;
        fs::remove_file(tmp_file_path)?;
        Ok(())
    }
    pub fn reset_encoder(&mut self) -> AudioCodecResult<()> {
        let tmp_file_path = self.app_folder.join("record_tmp");
        if tmp_file_path.is_file() {
            fs::remove_file(&tmp_file_path)?;
        }
        let tmp_file = fs::File::create_new(tmp_file_path)?;
        self.file_bit_writer = Some(BitWriter::new(BufWriter::new(tmp_file)));
        self.frames = 0;
        self.indicies.clear();
        self.samples_cache.clear();
        self.scratch_buf.fill(0.0);
        self.last_half.fill(0.0);
        Ok(())
    }
}
const MAX_K_BITS_LEN: u32 = 4;
struct CodecHeaderManager {}
impl CodecHeaderManager {
    fn new() -> Self {
        CodecHeaderManager {}
    }
    fn _write_skip_table(
        &self,
        bw: &mut BitWriter<File, LittleEndian>,
        skip_flags: &[Vec<bool>],
    ) -> AudioCodecResult<()> {
        warn!("write frame len{}", skip_flags.len() as u32);
        bw.write(4 * 8, skip_flags.len() as u32)?;
        for flags in skip_flags {
            for b in flags {
                bw.write_bit(*b)?;
            }
        }
        Ok(())
    }
    fn write_frames_len(
        &self,
        bw: &mut BitWriter<BufWriter<File>, LittleEndian>,
        frames_len: u64,
    ) -> AudioCodecResult<()> {
        bw.write(4 * 8, frames_len as u32)?;
        Ok(())
    }
    fn write_delta_indices(
        &self,
        bw: &mut BitWriter<BufWriter<File>, LittleEndian>,
        delta_indices: &Vec<Vec<u8>>,
    ) -> AudioCodecResult<()> {
        for frame_indices in delta_indices {
            for idx in frame_indices {
                bw.write(6, (*idx) as u32)?;
            }
        }
        Ok(())
    }
    fn write_sample_rate(
        &self,
        bw: &mut BitWriter<BufWriter<File>, LittleEndian>,
        rate: u32,
    ) -> AudioCodecResult<()> {
        bw.write(32, rate)?;
        Ok(())
    }
    fn write_channels(
        &self,
        bw: &mut BitWriter<BufWriter<File>, LittleEndian>,
        channels: u8,
    ) -> AudioCodecResult<()> {
        bw.write(8, channels)?;
        Ok(())
    }
    fn read_frames_len(
        &self,
        br: &mut BitReader<BufReader<File>, LittleEndian>,
    ) -> AudioCodecResult<u32> {
        let frames_len = br.read::<u32>(4 * 8)?;
        Ok(frames_len)
    }
    fn read_sample_rate(
        &self,
        br: &mut BitReader<BufReader<File>, LittleEndian>,
    ) -> AudioCodecResult<u32> {
        let sample_rate = br.read::<u32>(4 * 8)?;
        Ok(sample_rate)
    }
    fn read_channels(
        &self,
        br: &mut BitReader<BufReader<File>, LittleEndian>,
    ) -> AudioCodecResult<u8> {
        let channels = br.read::<u8>(8)?;
        Ok(channels)
    }
    fn read_delta_indices(
        &self,
        frame_len: u32,
        br: &mut BitReader<BufReader<File>, LittleEndian>,
        bands: &[Range<usize>],
    ) -> AudioCodecResult<Vec<Vec<usize>>> {
        let mut delta_indices = vec![vec![0_usize; bands.len()]; frame_len as usize];
        for indices in &mut delta_indices {
            for mut_delta_idx in indices {
                *mut_delta_idx = br.read::<u32>(6)? as usize;
            }
        }
        Ok(delta_indices)
    }
    fn write_remainder_len(
        &self,
        bw: &mut BitWriter<BufWriter<File>, LittleEndian>,
        remainder_len: u32,
    ) -> AudioCodecResult<()> {
        bw.write(MAX_K_BITS_LEN, remainder_len)?;

        Ok(())
    }
    fn read_remainder_len(
        &self,
        br: &mut BitReader<BufReader<File>, LittleEndian>,
    ) -> AudioCodecResult<u32> {
        let res = br.read(MAX_K_BITS_LEN)?;
        Ok(res)
    }
}
struct QuantizedAndDeltaIndices {
    data: Vec<i32>,
    indices: Vec<Vec<usize>>,
}
pub struct TinyDecoder {
    bands: Vec<Range<usize>>,
    file_bit_reader: Option<BitReader<BufReader<File>, LittleEndian>>,
    codec_header_manager: CodecHeaderManager,
    delta_table: Vec<f32>,
    file_header: FileHeader,
    decoded_frame_len: u32,
    mdct_via_dct4: MdctViaDct4<f32>,
    scratch: Vec<f32>,
    last_half: Vec<f32>,
    single_frame_data_buf: Vec<i32>,
    dequantized_buf: Vec<f32>,
    output_l_buf: Vec<f32>,
    output_r_buf: Vec<f32>,
    scale_factor: f32,
}
impl TinyDecoder {
    pub fn new() -> AudioCodecResult<Self> {
        let codec_header_manager = CodecHeaderManager::new();
        let mut delta_table = vec![0.0_f32; 64];
        let base_d = 4e-7_f32;
        let ratio = 1.24_f32;
        for idx in 0..64 {
            delta_table[idx as usize] = base_d * ratio.powi(idx);
        }
        let mut dct_planner = DctPlanner::<f32>::new();
        let plan_dct4 = dct_planner.plan_dct4(DATA_FRAME_SIZE / 2);
        let mdct_via_dct4 =
            rustdct::mdct::MdctViaDct4::new(plan_dct4, rustdct::mdct::window_fn::vorbis::<f32>);
        let scratch = vec![0.0; DATA_FRAME_SIZE];
        let last_half = vec![0.0_f32; DATA_FRAME_SIZE / 2];
        let scale_factor = (2.0 / DATA_FRAME_SIZE as f32).sqrt();
        Ok(Self {
            file_bit_reader: None,
            codec_header_manager,
            delta_table,
            bands: vec![
                0..2,
                2..4,
                4..8,
                8..16,
                16..32,
                32..64,
                64..128,
                128..256,
                256..512,
            ],
            file_header: FileHeader::new(0, 0, 0, vec![]),
            decoded_frame_len: 0,
            mdct_via_dct4,
            scratch,
            last_half,
            single_frame_data_buf: vec![0; DATA_FRAME_SIZE / 2],
            dequantized_buf: vec![0.0; DATA_FRAME_SIZE / 2],
            output_l_buf: vec![0.0; DATA_FRAME_SIZE / 2],
            output_r_buf: vec![0.0; DATA_FRAME_SIZE / 2],
            scale_factor,
        })
    }
    // pub fn decode(&mut self) -> AudioCodecResult<()> {
    //     let compressed_and_band_indices = self.load_encoded_data_from_file()?;
    //     let dequantized = self.dequantize(
    //         compressed_and_band_indices.data,
    //         compressed_and_band_indices.indices,
    //     )?;
    //     let pcm = self.apply_imdct(dequantized)?;
    //     let pcm_samples = pcm
    //         .into_iter()
    //         .map(|frame| {
    //             frame
    //                 .into_iter()
    //                 .map(|f| ((f * i16::MAX as f32).round() as i16).clamp(i16::MIN, i16::MAX))
    //                 .collect::<Vec<i16>>()
    //         })
    //         .collect::<Vec<Vec<i16>>>();
    //     self.save_wav(pcm_samples)?;
    //     Ok(())
    // }
    // fn load_encoded_data_from_file(&mut self) -> AudioCodecResult<QuantizedAndDeltaIndices> {
    //     let mut data = vec![];
    //     let frames_len = self
    //         .codec_header_manager
    //         .read_frames_len(&mut self.file_bit_reader)? as usize;
    //     let delta_indices = self.codec_header_manager.read_delta_indices(
    //         frames_len,
    //         &mut self.file_bit_reader,
    //         &self.bands,
    //     )?;
    //     for _frame_idx in 0..frames_len {
    //         let mut single_frame_data = vec![0; DATA_FRAME_SIZE / 2];
    //         for band_idx in 0..self.bands.len() {
    //             let remainder_bits_len = self
    //                 .codec_header_manager
    //                 .read_remainder_len(&mut self.file_bit_reader)?;
    //             for sample_idx in self.bands[band_idx].clone() {
    //                 single_frame_data[sample_idx] = self.read_unit_data(remainder_bits_len)?;
    //             }
    //         }
    //         data.push(single_frame_data);
    //     }
    //     Ok(QuantizedAndDeltaIndices {
    //         data,
    //         indices: delta_indices,
    //     })
    // }
    fn read_unit_data(
        bit_reader: &mut BitReader<BufReader<File>, LittleEndian>,
        remainder_bits_len: u32,
    ) -> AudioCodecResult<i32> {
        let quotient = bit_reader.read_unary1()?;
        let remainder = bit_reader.read::<u32>(remainder_bits_len)?;
        let combined = (quotient << remainder_bits_len) as i32 + remainder as i32;
        let original_value = (combined >> 1) ^ -(combined & 1);
        Ok(original_value)
    }
    fn dequantize(&mut self) -> AudioCodecResult<()> {
        // info!("into dequantize");

        for (band_idx, delta_idx) in self.file_header.delta_indices[self.decoded_frame_len as usize]
            .iter()
            .enumerate()
        {
            let delta = self.delta_table[*delta_idx];
            for k in self.bands[band_idx].clone() {
                let raw = self.single_frame_data_buf[k];
                if raw == 0 {
                    self.dequantized_buf[k] = 0.0;
                } else {
                    self.dequantized_buf[k] = (raw.abs() as f32) * delta * (raw.signum() as f32);
                }
            }
        }

        Ok(())
    }
    fn apply_imdct(&mut self) -> AudioCodecResult<Vec<f32>> {
        // let rec = rerun::RecordingStreamBuilder::new("mdct_debug")
        //     .spawn()
        //     .map_err(AudioCodecError::from_err)?;
        // let mut spectrogram: VecDeque<Vec<f32>> = VecDeque::new();
        // const MAX_SPECTROGRAM_LEN: usize = 300;
        self.output_l_buf.fill(0.0);
        self.output_r_buf.fill(0.0);
        self.mdct_via_dct4.process_imdct_with_scratch(
            &self.dequantized_buf,
            &mut self.output_l_buf,
            &mut self.output_r_buf,
            &mut self.scratch,
        );

        // rec.set_time_sequence("frame_idx", (offset / DATA_FRAME_SIZE) as i64);

        // let db_output = mdct_output
        //     .iter()
        //     .map(|&x| {
        //         let db = 20.0 * (x.abs() + 1e-6).log10();
        //         ((db + 80.0) / 80.0).clamp(0.0, 1.0)
        //     })
        //     .collect::<Vec<f32>>();
        // if offset % (DATA_FRAME_SIZE * 100) == 0 {
        // info!("Progress: {}/{}", offset, samples.len());
        // let flat = spectrogram.iter().flatten().copied().collect::<Vec<_>>();
        // let array = Array2::from_shape_vec((spectrogram.len(), DATA_FRAME_SIZE / 2), flat)
        //     .map_err(AudioCodecError::from_err)?;
        // rec.log(
        //     "audio/spectrogram",
        //     &Tensor::try_from(array).map_err(AudioCodecError::from_err)?,
        // )
        // .map_err(AudioCodecError::from_err)?;
        // }
        // spectrogram.push_back(db_output);
        // if spectrogram.len() > MAX_SPECTROGRAM_LEN {
        //     spectrogram.pop_front();
        // }

        for idx in 0..(DATA_FRAME_SIZE / 2) {
            let sample = (self.output_l_buf[idx] + self.last_half[idx]) * self.scale_factor;
            let final_sample = if sample.is_nan() {
                0.0
            } else {
                sample.clamp(-1.0, 1.0)
            };
            self.output_l_buf[idx] = final_sample;
        }
        let out_put = self.output_l_buf.clone();
        self.last_half = self.output_r_buf.clone();
        // info!("imdct completed!");
        Ok(out_put)
    }
    fn _save_wav(&self, pcm: Vec<Vec<i16>>) -> AudioCodecResult<()> {
        let wav_spec = WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut wav_writer =
            hound::WavWriter::new(File::create_new("./test_audio_file.wav")?, wav_spec)?;
        for frame in &pcm {
            for sample in frame {
                wav_writer.write_sample(*sample)?;
            }
        }
        Ok(())
    }
    pub fn reset_input_file(&mut self, file_path: PathBuf) -> AudioCodecResult<()> {
        let buf_reader = BufReader::with_capacity(1024 * 1024, File::open(file_path)?);
        let bit_reader = BitReader::<_, LittleEndian>::new(buf_reader);
        self.file_bit_reader = Some(bit_reader);
        self.last_half.fill(0.0);
        self.scratch.fill(0.0);
        self.decoded_frame_len = 0;
        Ok(())
    }
    pub fn read_file_header(&mut self) -> AudioCodecResult<()> {
        let bit_reader = self
            .file_bit_reader
            .as_mut()
            .ok_or(anyhow::Error::msg("file bit reader is none"))?;
        let frames_len = self.codec_header_manager.read_frames_len(bit_reader)?;
        let sample_rate = self.codec_header_manager.read_sample_rate(bit_reader)?;
        let channels = self.codec_header_manager.read_channels(bit_reader)?;
        let delta_indices =
            self.codec_header_manager
                .read_delta_indices(frames_len, bit_reader, &self.bands)?;
        self.file_header = FileHeader::new(frames_len, sample_rate, channels, delta_indices);
        // info!("file header:{:?}", self.file_header);
        Ok(())
    }
    pub fn pop_frame(&mut self) -> AudioCodecResult<(Vec<f32>, &FileHeader)> {
        if self.file_header.frames_len <= self.decoded_frame_len {
            return Err(anyhow::Error::msg("read file finished"));
        }
        let bit_reader = self
            .file_bit_reader
            .as_mut()
            .ok_or(anyhow::Error::msg("file bit reader is none"))?;
        for band_idx in 0..self.bands.len() {
            let remainder_bits_len = self.codec_header_manager.read_remainder_len(bit_reader)?;
            for sample_idx in self.bands[band_idx].clone() {
                self.single_frame_data_buf[sample_idx] =
                    Self::read_unit_data(bit_reader, remainder_bits_len)?;
            }
        }
        self.dequantize()?;
        // let now = Instant::now();
        let frame = self.apply_imdct()?;
        info!("debug sample val:{}", frame[0]);
        // info!(
        //     "decode 1 frame consumed :{}micros",
        //     (Instant::now() - now).as_micros()
        // );
        self.decoded_frame_len += 1;

        Ok((frame, &self.file_header))
    }
}
fn _analyze_pcm_energy(pcm: &[f32]) {
    let mut max_amp: f32 = 0.0;
    let mut sum_sq: f32 = 0.0;

    for &sample in pcm {
        let abs_s = sample.abs();
        if abs_s > max_amp {
            max_amp = abs_s;
        }
        sum_sq += sample * sample;
    }

    let rms = (sum_sq / pcm.len() as f32).sqrt();
    let db_peak = 20.0 * max_amp.log10();
    let db_rms = 20.0 * rms.log10();

    println!(
        " Peak: {:.4} ({:.2} dB), RMS: {:.4} ({:.2} dB)",
        max_amp, db_peak, rms, db_rms
    );
}
#[derive(Debug)]
pub struct FileHeader {
    frames_len: u32,
    sample_rate: u32,
    channels: u8,
    delta_indices: Vec<Vec<usize>>,
}
impl FileHeader {
    fn new(
        frames_len: u32,
        sample_rate: u32,
        channels: u8,
        delta_indices: Vec<Vec<usize>>,
    ) -> Self {
        Self {
            frames_len,
            sample_rate,
            channels,
            delta_indices,
        }
    }
    pub fn channels(&self) -> u8 {
        self.channels
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
#[cfg(test)]
mod test {}
