use super::*;
use crate::audio::decode::ClipDecoder;
use std::{fs::File, path::Path};

fn directory() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/recording/files");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

#[test]
fn recording_formats_encode_real_audio_that_the_existing_player_decodes() {
    let dir = directory();
    let frames: Vec<_> = (0..4800)
        .map(|n| {
            let value = (n as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 0.25;
            [value, -value]
        })
        .collect();
    for format in RecordingFormat::ALL {
        let path = dir.path().join(format!("clip.{}", format.extension()));
        let mut encoder = encoder::Encoder::create(&path, format).unwrap();
        encoder.write(&frames).unwrap();
        encoder.finish().unwrap();
        let mut decoder = ClipDecoder::open(
            Box::new(File::open(&path).unwrap()),
            Some(format.extension()),
        )
        .unwrap();
        assert_eq!((decoder.rate, decoder.channels), (48000, 2));
        let mut decoded = Vec::new();
        while let Some(samples) = decoder.next_samples().unwrap() {
            decoded.extend_from_slice(samples);
        }
        assert_eq!(decoded.len(), frames.len() * 2, "{format:?}");
        let error = decoded
            .iter()
            .zip(frames.iter().flatten())
            .map(|(actual, expected)| (actual - expected).abs())
            .sum::<f32>()
            / decoded.len() as f32;
        assert!(
            error
                < if format == RecordingFormat::Ogg {
                    0.02
                } else {
                    0.000001
                },
            "{format:?}: {error}"
        );
    }
}

#[test]
fn recording_encoder_never_overwrites_existing_files() {
    let dir = directory();
    let path = dir.path().join("existing.flac");
    std::fs::write(&path, b"original").unwrap();
    assert!(encoder::Encoder::create(&path, RecordingFormat::Flac).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"original");
}

#[test]
fn seeking_recorded_formats_preserves_crop_position_and_sample_content() {
    let dir = directory();
    let frames: Vec<_> = (0..96_000)
        .map(|n| {
            let sample = (n as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 0.25;
            [sample, -sample]
        })
        .collect();
    for format in RecordingFormat::ALL {
        let path = dir.path().join(format!("seek.{}", format.extension()));
        let mut encoder = encoder::Encoder::create(&path, format).unwrap();
        encoder.write(&frames).unwrap();
        encoder.finish().unwrap();
        let open = || {
            ClipDecoder::open(
                Box::new(File::open(&path).unwrap()),
                Some(format.extension()),
            )
            .unwrap()
        };
        let collect = |decoder: &mut ClipDecoder, mut position: u64| {
            let mut output = Vec::new();
            while output.len() < 4096 {
                let Some(samples) = decoder.next_samples().unwrap() else {
                    break;
                };
                for frame in samples.chunks_exact(2) {
                    if position >= 72_000 && output.len() < 4096 {
                        output.extend_from_slice(frame);
                    }
                    position += 1;
                }
            }
            output
        };
        let expected = collect(&mut open(), 0);
        let mut seeked = open();
        seeked.seek_to_frame(72_000).unwrap();
        let position = seeked.first_frame;
        let actual = collect(&mut seeked, position);
        assert_eq!(actual.len(), expected.len(), "{format}");
        let error = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / actual.len() as f32;
        assert!(error < 0.001, "{format}: seek error {error}");
    }
}

#[test]
fn recording_worker_drains_and_finalizes_unique_files_and_cancels_empty_capture() {
    let dir = directory();
    let mut paths = Vec::new();
    for _ in 0..2 {
        let (mut job, mut samples) =
            writer::WriterJob::start(dir.path().into(), RecordingFormat::Flac).unwrap();
        job.ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap();
        for _ in 0..480 {
            samples.push([0.25, -0.25]).unwrap();
        }
        drop(samples);
        job.stop();
        let path = job
            .result
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .unwrap();
        let mut decoder =
            ClipDecoder::open(Box::new(File::open(&path).unwrap()), Some("flac")).unwrap();
        let mut count = 0;
        while let Some(samples) = decoder.next_samples().unwrap() {
            count += samples.len();
        }
        assert_eq!(count, 960);
        paths.push(path);
    }
    assert_ne!(paths[0], paths[1]);
    let (mut job, samples) =
        writer::WriterJob::start(dir.path().into(), RecordingFormat::Wav).unwrap();
    job.ready
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap();
    drop(samples);
    job.stop();
    assert!(
        job.result
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn recording_worker_reports_directory_and_overflow_failures() {
    let dir = directory();
    let invalid = dir.path().join("file");
    std::fs::write(&invalid, b"keep").unwrap();
    let (job, _) = writer::WriterJob::start(invalid, RecordingFormat::Flac).unwrap();
    assert!(
        job.ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .is_err()
    );
    let (job, mut samples) =
        writer::WriterJob::start(dir.path().into(), RecordingFormat::Wav).unwrap();
    job.ready
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap();
    samples.push([0.5; 2]).unwrap();
    job.fault
        .store(writer::QUEUE_OVERFLOW, std::sync::atomic::Ordering::Release);
    assert!(
        job.result
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
            .contains("keep up")
    );
}
