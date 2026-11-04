use super::*;
use std::io::Cursor;

fn wav(channels: u16, samples: &[i16]) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + data_size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(channels.to_le_bytes());
    bytes.extend(48000_u32.to_le_bytes());
    bytes.extend((48000_u32 * u32::from(channels) * 2).to_le_bytes());
    bytes.extend((channels * 2).to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data_size.to_le_bytes());
    for sample in samples {
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}

#[test]
fn decodes_mono_pcm_with_correct_rate_and_normalization() {
    let mut decoder =
        ClipDecoder::open(Box::new(Cursor::new(wav(1, &[-32768, 0, 16384]))), None).unwrap();
    assert_eq!((decoder.rate, decoder.channels), (48000, 1));
    assert_eq!(decoder.next_samples().unwrap().unwrap(), &[-1.0, 0.0, 0.5]);
    assert!(decoder.next_samples().unwrap().is_none());
}

#[test]
fn preserves_stereo_channel_order() {
    let mut decoder = ClipDecoder::open(
        Box::new(Cursor::new(wav(2, &[16384, -16384, 0, 8192]))),
        None,
    )
    .unwrap();
    assert_eq!(decoder.channels, 2);
    assert_eq!(
        decoder.next_samples().unwrap().unwrap(),
        &[0.5, -0.5, 0.0, 0.25]
    );
}

#[test]
fn rejects_invalid_files_and_multichannel_clips() {
    assert!(ClipDecoder::open(Box::new(Cursor::new(b"not a audio file".to_vec())), None).is_err());
    assert!(ClipDecoder::open(Box::new(Cursor::new(wav(6, &[0; 12]))), None).is_err());
}

#[test]
fn decodes_flac_by_content() {
    let bytes = include_bytes!("../../../tests/fixtures/audio/tone.flac");
    let mut decoder = ClipDecoder::open(Box::new(Cursor::new(bytes.to_vec())), None).unwrap();
    assert_eq!((decoder.rate, decoder.channels), (44100, 1));
    let mut frames = 0;
    while let Some(samples) = decoder.next_samples().unwrap() {
        assert!(samples.iter().all(|sample| sample.is_finite()));
        frames += samples.len();
    }
    assert_eq!(frames, 1764);
}

#[test]
fn decodes_supported_containers_and_codecs() {
    for name in [
        "tone.mp3",
        "tone.m4a",
        "tone-alac.m4a",
        "tone.ogg",
        "tone.aiff",
        "tone.caf",
        "tone.mka",
        "tone.webm",
        "tone-adpcm.wav",
        "tone.mp2",
        "tone.aac",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio")
            .join(name);
        let mut decoder = ClipDecoder::open(
            Box::new(std::fs::File::open(&path).unwrap()),
            path.extension().and_then(|e| e.to_str()),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((decoder.rate, decoder.channels), (44100, 1), "{name}");
        let mut count = 0;
        while let Some(samples) = decoder
            .next_samples()
            .unwrap_or_else(|e| panic!("{name}: {e}"))
        {
            assert!(samples.iter().all(|s| s.is_finite()), "{name}");
            count += samples.len();
        }
        assert!(count >= 4000, "{name}: {count}");
    }
}

#[test]
fn decodes_opus_in_ogg_and_webm() {
    for name in ["tone-opus.ogg", "tone-opus.webm"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio")
            .join(name);
        let mut decoder = ClipDecoder::open(
            Box::new(std::fs::File::open(&path).unwrap()),
            path.extension().and_then(|e| e.to_str()),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((decoder.rate, decoder.channels), (48000, 2));
        let mut frames = 0;
        let mut peak = 0.0_f32;
        while let Some(samples) = decoder.next_samples().unwrap() {
            assert!(samples.iter().all(|s| s.is_finite()));
            frames += samples.len() / 2;
            peak = samples
                .iter()
                .fold(peak, |peak, sample| peak.max(sample.abs()));
        }
        assert!((4700..=6000).contains(&frames), "{name}: {frames}");
        assert!(peak > 0.01, "{name} decoded to silence");
    }
}

thread_local! {
    static MP3_WARNINGS: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

struct DecodeTestLogger;

impl log::Log for DecodeTestLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.target() == "symphonia_bundle_mp3::layer3" && record.level() == log::Level::Warn {
            MP3_WARNINGS.with_borrow_mut(|warnings| {
                if let Some(warnings) = warnings {
                    warnings.push(record.args().to_string());
                }
            });
        }
    }

    fn flush(&self) {}
}

#[test]
fn mp3_seek_preserves_reservoir_and_crop_samples() {
    static LOGGER: DecodeTestLogger = DecodeTestLogger;
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(log::LevelFilter::Warn);
    });
    let path = std::env::var_os("RASPIBOARD_MP3_SEEK_INPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/seek.mp3")
        });
    let start_ms = std::env::var("RASPIBOARD_MP3_SEEK_START_MS")
        .map(|value| value.parse::<u64>().unwrap())
        .unwrap_or(1500);
    let open = || ClipDecoder::open(Box::new(std::fs::File::open(&path).unwrap()), None).unwrap();
    let mut linear = open();
    let start = start_ms * u64::from(linear.rate) / 1000;
    let channels = linear.channels;
    let collect = |decoder: &mut ClipDecoder, mut position: u64| {
        let mut selected = Vec::new();
        while selected.len() < 2048 {
            let Some(samples) = decoder.next_samples().unwrap() else {
                break;
            };
            for frame in samples.chunks_exact(channels) {
                if position >= start && selected.len() < 2048 {
                    selected.extend_from_slice(frame);
                }
                position += 1;
            }
        }
        selected
    };
    let before = std::time::Instant::now();
    let expected = collect(&mut linear, 0);
    let linear_ms = before.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(expected.len(), 2048);
    let mut seeked = open();
    MP3_WARNINGS.with_borrow_mut(|warnings| *warnings = Some(Vec::new()));
    let before = std::time::Instant::now();
    seeked.seek_to_frame(start).unwrap();
    let first_frame = seeked.first_frame;
    let selected = collect(&mut seeked, first_frame);
    let seek_ms = before.elapsed().as_secs_f64() * 1000.0;
    let warnings = MP3_WARNINGS.with_borrow_mut(|warnings| warnings.take().unwrap());
    assert_eq!(selected.len(), expected.len());
    let error = selected
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / selected.len() as f32;
    eprintln!(
        "mp3 crop={} start_ms={start_ms} linear_ms={linear_ms:.2} seek_ms={seek_ms:.2} first_frame={first_frame} error={error:.8} warnings={warnings:?}",
        path.display()
    );
    assert!(error < 0.00001, "seeking changed crop audio by {error}");
    assert!(
        warnings.is_empty(),
        "seeking decoded packets without their bit reservoir: {warnings:?}"
    );
}
