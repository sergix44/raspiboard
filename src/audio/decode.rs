use std::sync::LazyLock;
use symphonia::core::{
    codecs::{audio::AudioDecoder, registry::CodecRegistry},
    formats::{FormatReader, TrackType, probe::Hint},
    io::{MediaSource, MediaSourceStream},
};

static CODECS: LazyLock<CodecRegistry> = LazyLock::new(|| {
    let mut codecs = CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut codecs);
    codecs.register_audio_decoder::<symphonia_adapter_libopus::OpusDecoder>();
    codecs
});

pub struct ClipDecoder {
    pub rate: u32,
    pub channels: usize,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    samples: Vec<f32>,
    pending: bool,
}

impl ClipDecoder {
    pub fn open(source: Box<dyn MediaSource>, extension: Option<&str>) -> Result<Self, String> {
        let source = MediaSourceStream::new(source, Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = extension {
            hint.with_extension(extension);
        }
        let format = symphonia::default::get_probe()
            .probe(&hint, source, Default::default(), Default::default())
            .map_err(|e| format!("Cannot read this audio file: {e}"))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or("The file has no audio track.")?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or("The file has no audio format information.")?;
        let decoder = CODECS
            .make_audio_decoder(params, &Default::default())
            .map_err(|e| format!("Unsupported audio encoding: {e}"))?;
        let track = track.id;
        let mut clip = Self {
            rate: 0,
            channels: 0,
            format,
            decoder,
            track,
            samples: Vec::new(),
            pending: false,
        };
        if clip.next_samples()?.is_none() {
            return Err("The audio file contains no audio samples.".into());
        }
        clip.pending = true;
        Ok(clip)
    }

    pub fn next_samples(&mut self) -> Result<Option<&[f32]>, String> {
        if self.pending {
            self.pending = false;
            return Ok(Some(&self.samples));
        }
        while let Some(packet) = self
            .format
            .next_packet()
            .map_err(|e| format!("Cannot read audio: {e}"))?
        {
            if packet.track_id != self.track {
                continue;
            }
            let audio = self
                .decoder
                .decode(&packet)
                .map_err(|e| format!("Cannot decode audio: {e}"))?;
            if audio.samples_interleaved() == 0 {
                continue;
            }
            if self.rate == 0 {
                self.rate = audio.spec().rate();
                self.channels = audio.spec().channels().count();
                if !(1..=384000).contains(&self.rate) || !(1..=2).contains(&self.channels) {
                    return Err("Only mono or stereo audio up to 384 kHz is supported.".into());
                }
            }
            if audio.spec().rate() != self.rate || audio.spec().channels().count() != self.channels
            {
                return Err("The audio format changed during playback.".into());
            }
            self.samples.resize(audio.samples_interleaved(), 0.0);
            audio.copy_to_slice_interleaved(&mut self.samples);
            return Ok(Some(&self.samples));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
