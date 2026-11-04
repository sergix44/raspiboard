use super::*;

#[test]
#[ignore = "Run with RASPIBOARD_PROFILE_INPUT pointing to a local JSON profile fixture"]
fn profile_audio_preparation_and_waveform() {
    let fixture = std::env::var("RASPIBOARD_PROFILE_INPUT").expect("profile fixture path");
    let entries: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    for entry in entries.as_array().unwrap() {
        let path = std::path::PathBuf::from(entry["path"].as_str().unwrap());
        let options = PlaybackOptions {
            start_ms: entry["start_ms"].as_u64(),
            end_ms: entry["end_ms"].as_u64(),
            relative_volume: None,
        };
        let cancelled = AtomicBool::new(false);
        let start = std::time::Instant::now();
        let waveform = crate::audio::waveform::analyze(&path, &cancelled).unwrap();
        let waveform_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = std::time::Instant::now();
        let (mut prepared, range) = prepare(path.clone(), options, &cancelled).unwrap();
        let preparation_ms = start.elapsed().as_secs_f64() * 1000.0;
        let cache_start = std::time::Instant::now();
        let cached = crate::audio::waveform::analyze(&path, &cancelled).unwrap();
        let cached_ms = cache_start.elapsed().as_secs_f64() * 1000.0;
        assert!(std::sync::Arc::ptr_eq(&waveform, &cached));
        let mut linear = ClipDecoder::open(
            Box::new(File::open(&path).unwrap()),
            path.extension().and_then(|extension| extension.to_str()),
        )
        .unwrap();
        let channels = linear.channels;
        let collect = |decoder: &mut ClipDecoder, mut position: u64| {
            let mut selected = Vec::new();
            while selected.len() < 2048 {
                let Some(samples) = decoder.next_samples().unwrap() else {
                    break;
                };
                for frame in samples.chunks_exact(channels) {
                    if position >= range.start && selected.len() < 2048 {
                        selected.extend_from_slice(frame);
                    }
                    position += 1;
                }
            }
            selected
        };
        let expected = collect(&mut linear, 0);
        let position = prepared.first_frame;
        let selected = collect(&mut prepared, position);
        assert_eq!(selected.len(), expected.len());
        let error = selected
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / selected.len() as f32;
        assert!(
            error < 0.001,
            "seeking shifted decoded crop audio by {error}"
        );
        eprintln!(
            "profile codec={} duration_ms={} crop_start_ms={:?} crop_end_ms={:?} waveform_ms={:.2} preparation_ms={:.2} cached_waveform_ms={:.3} seek_error={:.8}",
            entry["extension"],
            waveform.duration_ms,
            options.start_ms,
            options.end_ms,
            waveform_ms,
            preparation_ms,
            cached_ms,
            error
        );
    }
}

#[test]
fn cropped_playback_uses_container_duration_and_seeks_near_the_start() {
    let source: Vec<_> = (0..96_000).map(|n| (n % 16000) as i16).collect();
    let (_directory, path) = source_file(&source);
    let options = PlaybackOptions {
        start_ms: Some(1500),
        end_ms: Some(1600),
        relative_volume: None,
    };
    let (mut decoder, range) = prepare(path, options, &AtomicBool::new(false)).unwrap();
    assert!(
        decoder.first_frame > 0,
        "the crop must seek rather than decode from the beginning"
    );
    assert!(decoder.first_frame <= range.start);
    let (producer, mut samples) = rtrb::RingBuffer::new(8192);
    produce(
        &mut decoder,
        &mut [producer],
        &AtomicBool::new(false),
        range,
    )
    .unwrap();
    let frames: Vec<_> = std::iter::from_fn(|| samples.pop().ok()).collect();
    assert_eq!(frames.len(), 4800);
    assert_eq!(frames[500], [source[72_500] as f32 / 32768.0; 2]);
}
use std::io::Cursor;

fn decoder(channels: u16, samples: &[i16]) -> ClipDecoder {
    ClipDecoder::open(Box::new(Cursor::new(wav_bytes(channels, samples))), None).unwrap()
}

fn wav_bytes(channels: u16, samples: &[i16]) -> Vec<u8> {
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
fn local_only_decode_finishes_beyond_queue_capacity_without_a_remote_consumer() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-results/local-playback");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let path = directory.path().join("long.wav");
    std::fs::write(&path, wav_bytes(1, &[16384; 32768])).unwrap();
    let job = DecodeJob::start(path, false, PlaybackOptions::default()).unwrap();
    let mut outputs = job
        .ready
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert!(outputs.remote.is_none());
    assert_eq!(outputs.duration_ms, Some(683));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut frames = 0;
    loop {
        let eof = outputs.local.eof.load(Ordering::Acquire);
        match outputs.local.samples.pop() {
            Ok(frame) => {
                let gain =
                    (frames as f32 / 240.0).min(1.0) * ((32767 - frames) as f32 / 240.0).min(1.0);
                assert_eq!(frame, [0.5 * gain; 2]);
                frames += 1;
            }
            Err(_) if eof => break,
            Err(_) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "local decoding stalled"
                );
                thread::yield_now();
            }
        }
    }
    assert_eq!(frames, 32768);
    assert!(
        outputs
            .local
            .result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_ok()
    );
}

#[test]
fn duration_analysis_finishes_without_consuming_the_playback_queue() {
    let (_directory, path) = source_file(&[16384; 48000]);
    let _decode = DecodeJob::start(path.clone(), false, PlaybackOptions::default()).unwrap();
    let duration = DurationJob::start(path).unwrap();
    assert_eq!(
        duration
            .ready
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        1000
    );
}

#[test]
fn delivers_every_stereo_frame_to_both_outputs_in_order() {
    let mut decoder = decoder(2, &[16384, -16384, 8192, 0, -8192, 32767]);
    let (remote, mut remote_samples) = rtrb::RingBuffer::new(8);
    let (local, mut local_samples) = rtrb::RingBuffer::new(8);
    produce(
        &mut decoder,
        &mut [remote, local],
        &AtomicBool::new(false),
        FrameRange::default(),
    )
    .unwrap();
    for (i, mut expected) in [[0.5, -0.5], [0.25, 0.0], [-0.25, 32767.0 / 32768.0]]
        .into_iter()
        .enumerate()
    {
        for sample in &mut expected {
            *sample *= i as f32 / 240.0;
            *sample *= (2 - i) as f32 / 240.0;
        }
        assert_eq!(remote_samples.pop().unwrap(), expected);
        assert_eq!(local_samples.pop().unwrap(), expected);
    }
    assert!(remote_samples.pop().is_err());
    assert!(local_samples.pop().is_err());
}

#[test]
fn duplicates_mono_for_each_output() {
    let mut decoder = decoder(1, &[16384, -8192]);
    let (remote, mut remote_samples) = rtrb::RingBuffer::new(4);
    let (local, mut local_samples) = rtrb::RingBuffer::new(4);
    produce(
        &mut decoder,
        &mut [remote, local],
        &AtomicBool::new(false),
        FrameRange::default(),
    )
    .unwrap();
    for expected in [[0.0, 0.0], [-0.0, -0.0]] {
        assert_eq!(remote_samples.pop().unwrap(), expected);
        assert_eq!(local_samples.pop().unwrap(), expected);
    }
}

#[test]
fn cancellation_releases_a_decoder_blocked_by_the_local_output() {
    let mut decoder = decoder(1, &[16384; 32]);
    let (remote, mut remote_samples) = rtrb::RingBuffer::new(2);
    let (local, _local_samples) = rtrb::RingBuffer::new(1);
    let cancel = AtomicBool::new(false);
    thread::scope(|scope| {
        let (done_tx, done) = mpsc::sync_channel(1);
        let cancel_ref = &cancel;
        scope.spawn(move || {
            let result = produce(
                &mut decoder,
                &mut [remote, local],
                cancel_ref,
                FrameRange::default(),
            );
            done_tx.send(result).unwrap();
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while remote_samples.slots() != 2 && std::time::Instant::now() < deadline {
            thread::yield_now();
        }
        cancel.store(true, Ordering::Release);
        assert!(done.recv_timeout(Duration::from_secs(2)).unwrap().is_ok());
        assert_eq!(remote_samples.pop().unwrap(), [0.0; 2]);
        let expected = 0.5 / 240.0 * (30.0 / 240.0);
        assert_eq!(remote_samples.pop().unwrap(), [expected; 2]);
    });
}

#[test]
fn fades_boundaries_without_changing_frame_count() {
    let mut decoder = decoder(1, &[16384; 1000]);
    let (producer, mut samples) = rtrb::RingBuffer::new(1001);
    produce(
        &mut decoder,
        &mut [producer],
        &AtomicBool::new(false),
        FrameRange::default(),
    )
    .unwrap();
    let frames: Vec<_> = std::iter::from_fn(|| samples.pop().ok()).collect();
    assert_eq!(frames.len(), 1000);
    assert_eq!(frames[0], [0.0; 2]);
    assert_eq!(frames[500], [0.5; 2]);
    assert_eq!(frames[999], [0.0; 2]);
    assert!(frames[..241].windows(2).all(|w| w[0][0] <= w[1][0]));
    assert!(frames[759..].windows(2).all(|w| w[0][0] >= w[1][0]));
}

fn failing_decoder() -> ClipDecoder {
    struct FailingSource(Cursor<Vec<u8>>);
    impl std::io::Read for FailingSource {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let remaining = 32768_usize.saturating_sub(self.0.position() as usize);
            if remaining == 0 {
                return Err(std::io::Error::other("injected read failure"));
            }
            let len = buf.len().min(remaining);
            std::io::Read::read(&mut self.0, &mut buf[..len])
        }
    }
    impl std::io::Seek for FailingSource {
        fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> {
            Err(std::io::Error::other("not seekable"))
        }
    }
    impl symphonia::core::io::MediaSource for FailingSource {
        fn is_seekable(&self) -> bool {
            false
        }
        fn byte_len(&self) -> Option<u64> {
            None
        }
    }
    let source = FailingSource(Cursor::new(wav_bytes(1, &[16384; 32768])));
    ClipDecoder::open(Box::new(source), Some("wav")).unwrap()
}

#[test]
fn read_error_fades_buffered_audio_before_reporting_failure() {
    let mut decoder = failing_decoder();
    let (producer, mut samples) = rtrb::RingBuffer::new(32768);
    let result = produce(
        &mut decoder,
        &mut [producer],
        &AtomicBool::new(false),
        FrameRange::default(),
    );
    assert!(result.unwrap_err().contains("injected read failure"));
    let frames: Vec<_> = std::iter::from_fn(|| samples.pop().ok()).collect();
    assert!(frames.len() > 240);
    assert_eq!(frames.last(), Some(&[0.0; 2]));
}

#[test]
fn crops_source_frames_across_packets_with_fades_on_the_selected_range() {
    let source: Vec<i16> = (0..9600).map(|i| (i * 3) as i16).collect();
    for (start_ms, end_ms, first, length) in [
        (Some(17), Some(73), 816, 2688),
        (Some(101), None, 4848, 4752),
        (None, Some(73), 0, 3504),
    ] {
        let mut decoder = decoder(1, &source);
        let (producer, mut samples) = rtrb::RingBuffer::new(10000);
        let range = FrameRange::new(
            PlaybackOptions {
                start_ms,
                end_ms,
                relative_volume: None,
            },
            48000,
        );
        produce(
            &mut decoder,
            &mut [producer],
            &AtomicBool::new(false),
            range,
        )
        .unwrap();
        let frames: Vec<_> = std::iter::from_fn(|| samples.pop().ok()).collect();
        assert_eq!(frames.len(), length);
        for (i, frame) in frames.into_iter().enumerate() {
            let envelope = (i as f32 / 240.0).min(1.0) * ((length - i - 1) as f32 / 240.0).min(1.0);
            let expected = source[first + i] as f32 / 32768.0 * envelope;
            assert!((frame[0] - expected).abs() < 1e-6, "wrong frame {i}");
            assert_eq!(frame[0], frame[1]);
        }
    }
}

#[test]
fn crop_uses_only_frames_whose_timestamps_are_inside_the_interval() {
    let mut bytes = wav_bytes(1, &[16384; 200]);
    bytes[24..28].copy_from_slice(&44100_u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&88200_u32.to_le_bytes());
    let mut decoder = ClipDecoder::open(Box::new(Cursor::new(bytes)), None).unwrap();
    let (producer, mut samples) = rtrb::RingBuffer::new(201);
    let range = FrameRange::new(
        PlaybackOptions {
            start_ms: Some(1),
            end_ms: Some(2),
            relative_volume: None,
        },
        44100,
    );
    produce(
        &mut decoder,
        &mut [producer],
        &AtomicBool::new(false),
        range,
    )
    .unwrap();
    assert_eq!(std::iter::from_fn(|| samples.pop().ok()).count(), 44);
}

fn source_file(samples: &[i16]) -> (tempfile::TempDir, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-results/audio-options");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root).unwrap();
    let path = directory.path().join("source.wav");
    std::fs::write(&path, wav_bytes(1, samples)).unwrap();
    (directory, path)
}

#[test]
fn rejects_crop_outside_the_actual_source_before_output_is_ready() {
    let (_directory, path) = source_file(&[16384; 4800]);
    for options in [
        PlaybackOptions {
            start_ms: Some(100),
            ..Default::default()
        },
        PlaybackOptions {
            end_ms: Some(101),
            ..Default::default()
        },
        PlaybackOptions {
            start_ms: Some(u64::MAX),
            ..Default::default()
        },
    ] {
        let job = DecodeJob::start(path.clone(), false, options).unwrap();
        assert!(
            job.ready
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_err(),
            "accepted {options:?}"
        );
    }
}

#[test]
fn job_queues_only_the_crop_and_carries_immutable_gain_to_both_outputs() {
    let (_directory, path) = source_file(&[16384; 4800]);
    let options = PlaybackOptions {
        start_ms: Some(10),
        end_ms: Some(30),
        relative_volume: Some(15.0),
    };
    let job = DecodeJob::start(path, true, options).unwrap();
    let mut outputs = job
        .ready
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    outputs
        .local
        .result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    let mut remote = outputs.remote.take().unwrap();
    assert!((remote.clip_gain - 5.623413).abs() < 0.00001);
    assert_eq!(remote.clip_gain, outputs.local.clip_gain);
    let remote_frames: Vec<_> = std::iter::from_fn(|| remote.samples.pop().ok()).collect();
    let local_frames: Vec<_> = std::iter::from_fn(|| outputs.local.samples.pop().ok()).collect();
    assert_eq!(local_frames.len(), 960);
    assert_eq!(remote_frames, local_frames);
    assert_eq!(local_frames[480], [0.5; 2]);
}

#[test]
fn cancellation_releases_a_cropped_decoder_blocked_by_an_output() {
    let mut decoder = decoder(1, &[16384; 1000]);
    let (producer, mut samples) = rtrb::RingBuffer::new(1);
    let cancel = AtomicBool::new(false);
    thread::scope(|scope| {
        let (done_tx, done) = mpsc::sync_channel(1);
        let cancel_ref = &cancel;
        scope.spawn(move || {
            let result = produce(
                &mut decoder,
                &mut [producer],
                cancel_ref,
                FrameRange {
                    start: 100,
                    end: Some(132),
                },
            );
            done_tx.send(result).unwrap();
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while samples.slots() == 0 && std::time::Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(samples.slots(), 1);
        cancel.store(true, Ordering::Release);
        assert!(done.recv_timeout(Duration::from_secs(2)).unwrap().is_ok());
        assert_eq!(samples.pop().unwrap(), [0.0; 2]);
    });
}

#[test]
fn cancellation_precedes_crop_validation_and_file_access() {
    let options = PlaybackOptions {
        start_ms: Some(u64::MAX),
        ..Default::default()
    };
    let error = prepare("missing.wav".into(), options, &AtomicBool::new(true))
        .err()
        .unwrap();
    assert!(error.contains("cancelled"));
}

#[test]
fn accepts_the_displayed_end_millisecond_for_a_fractional_duration() {
    let (_directory, path) = source_file(&[16384; 20]);
    let mut bytes = wav_bytes(1, &[16384; 20]);
    bytes[24..28].copy_from_slice(&44100_u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&88200_u32.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let job = DecodeJob::start(
        path.clone(),
        false,
        PlaybackOptions {
            end_ms: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let mut outputs = job
        .ready
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    outputs
        .local
        .result
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        std::iter::from_fn(|| outputs.local.samples.pop().ok()).count(),
        20
    );
    for options in [
        PlaybackOptions {
            start_ms: Some(1),
            ..Default::default()
        },
        PlaybackOptions {
            end_ms: Some(2),
            ..Default::default()
        },
    ] {
        let job = DecodeJob::start(path.clone(), false, options).unwrap();
        assert!(
            job.ready
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn crop_at_a_packet_boundary_does_not_read_errors_after_the_selected_interval() {
    let mut probe = failing_decoder();
    let mut available_frames = 0;
    while let Ok(Some(samples)) = probe.next_samples() {
        available_frames += samples.len() as u64;
    }
    assert!(available_frames > 240);
    let mut decoder = failing_decoder();
    let (producer, mut samples) = rtrb::RingBuffer::new(32768);
    produce(
        &mut decoder,
        &mut [producer],
        &AtomicBool::new(false),
        FrameRange {
            start: 0,
            end: Some(available_frames),
        },
    )
    .unwrap();
    assert_eq!(
        std::iter::from_fn(|| samples.pop().ok()).count() as u64,
        available_frames
    );
}
