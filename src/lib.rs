#[cfg(not(target_os = "linux"))]
compile_error!("Raspiboard platform services currently support Linux only.");
pub const APP_ID: &str = "io.raspiboard.Raspiboard";
#[cfg(target_os = "linux")]
pub mod audio;
#[cfg(target_os = "linux")]
pub mod hotkeys;
pub mod storage;
pub mod theme;
