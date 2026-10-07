use std::{collections::HashMap, fmt::Display};

use my_audio_codec::AudioCodecResult;

const WAVE_WAV: &[u8] = include_bytes!("../resources/wave_effect.wav");
const CICADA_CHIRPING_WAV: &[u8] = include_bytes!("../resources/cicada_chirping.wav");
const RAIN_WAV: &[u8] = include_bytes!("../resources/rain.wav");
const PIANO_WAV: &[u8] = include_bytes!("../resources/piano.wav");
pub struct EffectFilter {
    effect_map: HashMap<SelectedEffect, Box<[f32]>>,
    last_idx: usize,
}
impl EffectFilter {
    pub fn new() -> AudioCodecResult<Self> {
        let mut effect_map = HashMap::new();
        let wav_reader = hound::WavReader::new(WAVE_WAV)?;
        let samples = wav_reader.into_samples::<i16>();
        let wave_pcm = samples
            .map(|s| {
                if let Ok(s) = s {
                    ((s as f32) / i16::MAX as f32).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect::<Vec<f32>>();
        effect_map.insert(SelectedEffect::Wave, wave_pcm.into_boxed_slice());
        let wav_reader = hound::WavReader::new(CICADA_CHIRPING_WAV)?;
        let samples = wav_reader.into_samples::<i16>();
        let wave_pcm = samples
            .map(|s| {
                if let Ok(s) = s {
                    ((s as f32) / i16::MAX as f32).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect::<Vec<f32>>();
        effect_map.insert(SelectedEffect::CicadaChirping, wave_pcm.into_boxed_slice());
        let wav_reader = hound::WavReader::new(RAIN_WAV)?;
        let samples = wav_reader.into_samples::<i16>();
        let wave_pcm = samples
            .map(|s| {
                if let Ok(s) = s {
                    ((s as f32) / i16::MAX as f32).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect::<Vec<f32>>();
        effect_map.insert(SelectedEffect::Rain, wave_pcm.into_boxed_slice());
        let wav_reader = hound::WavReader::new(PIANO_WAV)?;
        let samples = wav_reader.into_samples::<i16>();
        let wave_pcm = samples
            .map(|s| {
                if let Ok(s) = s {
                    ((s as f32) / i16::MAX as f32).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect::<Vec<f32>>();
        effect_map.insert(SelectedEffect::Piano, wave_pcm.into_boxed_slice());
        let last_idx = 0;
        Ok(Self {
            effect_map,
            last_idx,
        })
    }
    pub fn filter(
        &mut self,
        mut pcm: Vec<f32>,
        effect: SelectedEffect,
    ) -> AudioCodecResult<Vec<f32>> {
        match &effect {
            SelectedEffect::None => Ok(pcm),
            _ => {
                let effect_pcm = &self.effect_map[&effect];
                for item in &mut pcm {
                    *item = (*item + (effect_pcm[self.last_idx])).clamp(-1.0, 1.0);
                    self.last_idx = (self.last_idx + 1) % effect_pcm.len();
                }
                Ok(pcm)
            }
        }
    }
}
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum SelectedEffect {
    None,
    Wave,
    CicadaChirping,
    Rain,
    Piano,
}
impl Display for SelectedEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelectedEffect::None => f.write_str("no effect")?,
            SelectedEffect::Wave => f.write_str("wave effect")?,
            SelectedEffect::CicadaChirping => f.write_str("cicada chirping effect")?,
            SelectedEffect::Rain => f.write_str("rain effect")?,
            SelectedEffect::Piano => f.write_str("piano effect")?,
        }
        Ok(())
    }
}
