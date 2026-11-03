use super::{RecordingFormat, SAMPLE_RATE};
use libloading::Library;
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    fs::{File, OpenOptions},
    os::fd::AsRawFd,
    path::Path,
    ptr::{self, NonNull},
};

#[repr(C)]
struct SoundInfo {
    frames: i64,
    samplerate: c_int,
    channels: c_int,
    format: c_int,
    sections: c_int,
    seekable: c_int,
}

type Open = unsafe extern "C" fn(c_int, c_int, *mut SoundInfo, c_int) -> *mut c_void;
type Write = unsafe extern "C" fn(*mut c_void, *const f32, i64) -> i64;
type Close = unsafe extern "C" fn(*mut c_void) -> c_int;
type Error = unsafe extern "C" fn(*mut c_void) -> c_int;
type ErrorText = unsafe extern "C" fn(c_int) -> *const c_char;
type Sync = unsafe extern "C" fn(*mut c_void);

struct Api {
    open: Open,
    write: Write,
    close: Close,
    error: Error,
    error_text: ErrorText,
    sync: Sync,
    _library: Library,
}

impl Api {
    fn load() -> Result<Self, String> {
        // Signatures and constants follow the stable libsndfile public C API.
        unsafe {
            let library = Library::new("libsndfile.so.1")
                .map_err(|e| format!("Cannot load recording codecs (libsndfile): {e}"))?;
            Ok(Self {
                open: *library.get(b"sf_open_fd\0").map_err(|e| e.to_string())?,
                write: *library
                    .get(b"sf_writef_float\0")
                    .map_err(|e| e.to_string())?,
                close: *library.get(b"sf_close\0").map_err(|e| e.to_string())?,
                error: *library.get(b"sf_error\0").map_err(|e| e.to_string())?,
                error_text: *library
                    .get(b"sf_error_number\0")
                    .map_err(|e| e.to_string())?,
                sync: *library.get(b"sf_write_sync\0").map_err(|e| e.to_string())?,
                _library: library,
            })
        }
    }

    fn message(&self, code: c_int) -> String {
        unsafe {
            CStr::from_ptr((self.error_text)(code))
                .to_string_lossy()
                .into_owned()
        }
    }
}

pub(super) struct Encoder {
    handle: Option<NonNull<c_void>>,
    api: Api,
    file: File,
    format: RecordingFormat,
    frames: u64,
}

impl Encoder {
    pub fn create(path: &Path, format: RecordingFormat) -> Result<Self, String> {
        let api = Api::load()?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut info = SoundInfo {
            frames: 0,
            samplerate: SAMPLE_RATE as c_int,
            channels: 2,
            format: match format {
                RecordingFormat::Flac => 0x170000 | 0x0003,
                RecordingFormat::Wav => 0x010000 | 0x0003,
                RecordingFormat::Ogg => 0x200000 | 0x0060,
            },
            sections: 0,
            seekable: 0,
        };
        let handle = NonNull::new(unsafe { (api.open)(file.as_raw_fd(), 0x20, &mut info, 0) });
        let Some(handle) = handle else {
            let message = api.message(unsafe { (api.error)(ptr::null_mut()) });
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(format!("Cannot create {format} recording: {message}"));
        };
        Ok(Self {
            handle: Some(handle),
            api,
            file,
            format,
            frames: 0,
        })
    }

    pub fn write(&mut self, frames: &[[f32; 2]]) -> Result<(), String> {
        let limit = frame_limit(self.format);
        if self.frames.saturating_add(frames.len() as u64) > limit {
            return Err(format!(
                "The {} recording reached its file size limit.",
                self.format
            ));
        }
        let handle = self.handle.unwrap().as_ptr();
        let count =
            unsafe { (self.api.write)(handle, frames.as_ptr().cast(), frames.len() as i64) };
        if count != frames.len() as i64 {
            return Err(format!(
                "Cannot write recording: {}",
                self.api.message(unsafe { (self.api.error)(handle) })
            ));
        }
        self.frames += count as u64;
        Ok(())
    }

    pub fn checkpoint(&mut self) -> Result<(), String> {
        let handle = self.handle.unwrap().as_ptr();
        unsafe {
            (self.api.sync)(handle);
        }
        let error = unsafe { (self.api.error)(handle) };
        if error == 0 {
            Ok(())
        } else {
            Err(self.api.message(error))
        }
    }

    pub fn finish(mut self) -> Result<(), String> {
        let handle = self.handle.take().unwrap();
        let error = unsafe { (self.api.close)(handle.as_ptr()) };
        if error != 0 {
            return Err(format!(
                "Cannot finish recording: {}",
                self.api.message(error)
            ));
        }
        self.file
            .sync_all()
            .map_err(|e| format!("Cannot save recording: {e}"))
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            unsafe {
                (self.api.close)(handle.as_ptr());
            }
        }
    }
}

fn frame_limit(format: RecordingFormat) -> u64 {
    match format {
        RecordingFormat::Wav => (u32::MAX as u64 - 4096) / 6,
        RecordingFormat::Flac => (1_u64 << 36) - 1,
        RecordingFormat::Ogg => i64::MAX as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_encoder_rejects_format_limits_before_writing() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/recording/limits");
        std::fs::create_dir_all(&root).unwrap();
        let dir = tempfile::tempdir_in(root).unwrap();
        for format in RecordingFormat::ALL {
            let path = dir.path().join(format!("limit.{}", format.extension()));
            let mut encoder = Encoder::create(&path, format).unwrap();
            encoder.frames = frame_limit(format) - 1;
            assert!(encoder.write(&[[0.0; 2]; 2]).unwrap_err().contains("limit"));
            encoder.finish().unwrap();
            assert!(std::fs::metadata(path).unwrap().len() < 8192);
        }
    }
}
