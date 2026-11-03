use flexi_logger::{
    Age, Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, FlexiLoggerError, LogSpecification,
    Logger, LoggerHandle, Naming,
};
use std::{io::Write, path::Path};

pub fn init(directory: &Path) -> Result<LoggerHandle, FlexiLoggerError> {
    let specification = LogSpecification::env_or_parse("warn,raspiboard=info")?;
    let handle = logger(directory, specification).start()?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!(target: "raspiboard::panic", "{info}");
        log::logger().flush();
        previous_hook(info);
    }));
    Ok(handle)
}

fn logger(directory: &Path, specification: LogSpecification) -> Logger {
    Logger::with(specification)
        .log_to_file(
            FileSpec::default()
                .directory(directory)
                .basename("raspiboard"),
        )
        .format(format)
        .duplicate_to_stderr(Duplicate::Warn)
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepForDays(7),
        )
}

fn format(
    writer: &mut dyn Write,
    now: &mut DeferredNow,
    record: &log::Record<'_>,
) -> std::io::Result<()> {
    write!(
        writer,
        "[{}] {}.{}: {}",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        record.target(),
        record.level(),
        record.args()
    )
}

#[cfg(test)]
mod tests;
