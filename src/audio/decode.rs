use std::sync::LazyLock;
use symphonia::core::{
    codecs::{audio::AudioDecoder, registry::CodecRegistry},
    formats::{FormatReader, SeekMode, SeekTo, TrackType, probe::Hint},
    io::{MediaSource, MediaSourceStream},
    units::{TimeBase, Timestamp},
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
    pub frames: Option<u64>,
    pub first_frame: u64,
    time_base: Option<TimeBase>,
    origin: Timestamp,
    seek_pending: bool,
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
        let frames = track.num_frames;
        let time_base = track.time_base;
        let track = track.id;
        let mut clip = Self {
            rate: 0,
            channels: 0,
            format,
            decoder,
            track,
            samples: Vec::new(),
            pending: false,
            frames,
            first_frame: 0,
            time_base,
            origin: Timestamp::ZERO,
            seek_pending: false,
        };
        if clip.next_samples()?.is_none() {
            return Err("The audio file contains no audio samples.".into());
        }
        clip.pending = true;
        Ok(clip)
    }

    pub fn seek_to_frame(&mut self, frame: u64) -> Result<(), String> {
        // Transform codecs need preceding packets to rebuild their overlap state.
        let preroll = frame.saturating_sub(u64::from(self.rate) / 10);
        if preroll == 0 {
            return Ok(());
        }
        let time_base = self
            .time_base
            .ok_or("This audio format has no seek timebase.")?;
        let ticks = u128::from(preroll) * u128::from(time_base.denom.get())
            / (u128::from(self.rate) * u128::from(time_base.numer.get()));
        let ts = i128::from(self.origin.get()) + ticks as i128;
        let ts = Timestamp::new(i64::try_from(ts).map_err(|_| "Crop start is too large.")?);
        let seeked = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Timestamp {
                    ts,
                    track_id: self.track,
                },
            )
            .map_err(|e| format!("Cannot seek to crop: {e}"))?;
        let position = ((i128::from(seeked.actual_ts.get()) - i128::from(self.origin.get())).max(0)
            as u128)
            * u128::from(time_base.numer.get())
            * u128::from(self.rate)
            / u128::from(time_base.denom.get());
        self.first_frame = u64::try_from(position).map_err(|_| "Crop position is too large.")?;
        if self.first_frame > frame {
            return Err("Audio seek skipped the crop start.".into());
        }
        self.decoder.reset();
        self.pending = false;
        self.samples.clear();
        self.seek_pending = true;
        if self.next_samples()?.is_none() {
            return Err("Crop start is beyond the audio file.".into());
        }
        self.pending = true;
        Ok(())
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
            if self.seek_pending {
                let timestamp = packet.pts.saturating_add(packet.trim_start);
                let base = self.time_base.ok_or("Missing seek timebase.")?;
                let frames = ((i128::from(timestamp.get()) - i128::from(self.origin.get())).max(0)
                    as u128)
                    * u128::from(base.numer.get())
                    * u128::from(self.rate)
                    / u128::from(base.denom.get());
                self.first_frame =
                    u64::try_from(frames).map_err(|_| "Crop position is too large.")?;
                self.seek_pending = false;
            }
            if self.rate == 0 {
                // Gapless decoding starts after the packet's leading encoder trim.
                self.origin = packet.pts.saturating_add(packet.trim_start);
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
