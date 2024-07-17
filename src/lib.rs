mod eventlog;

#[cfg(windows)]
pub use eventlog::EventLogLayer;
