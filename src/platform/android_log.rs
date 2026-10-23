use base::fs::android;
use hbb_common::{
    flexi_logger::{opt_format, writers::LogWriter, DeferredNow, Logger},
    log::{Level, LevelFilter, Log, Record},
    log_throttle::LogThrottle,
};
use std::{io, sync::Once, time::Duration};

const ERROR_INTERVAL: Duration = Duration::from_secs(60);

pub fn initialize() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let result = Logger::try_with_env_or_str("debug,reqwest=warn,hyper_util=warn,rustls=warn,webrtc-sctp=warn,webrtc=warn,webrtc_ice::agent::agent_internal=error,webrtc::peer_connection=error")
            .and_then(|logger| logger.log_to_writer(Box::new(DocumentWriter)).start());
        if let Err(error) = result {
            report_error(&error);
        }
    });
}

struct DocumentWriter;

impl LogWriter for DocumentWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> io::Result<()> {
        // JNI logs while detaching threads; calling back into JNI there is unsafe.
        if record.target() == "jni" || record.target().starts_with("jni::") {
            logcat().log(record);
            return Ok(());
        }
        let mut message = Vec::new();
        opt_format(&mut message, now, record)?;
        message.push(b'\n');
        let message = String::from_utf8(message)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let result = android::request(serde_json::json!({
            "op": "log_write", "message": message
        }));
        match result {
            Ok(value) if value.as_bool() == Some(true) => Ok(()),
            Ok(value) if value.as_bool() == Some(false) => {
                // Startup diagnostics precede the first user-granted log directory.
                logcat().log(record);
                Ok(())
            }
            result => {
                let error = match result {
                    Err(error) => error.to_string(),
                    _ => "Invalid document logger response".to_owned(),
                };
                report_error(&error);
                Err(io::Error::new(io::ErrorKind::Other, error))
            }
        }
    }

    fn flush(&self) -> io::Result<()> {
        // The Android writer writes directly to an unbuffered file stream.
        Ok(())
    }
}

fn logcat() -> android_logger::AndroidLogger {
    android_logger::AndroidLogger::new(
        android_logger::Config::default()
            .with_max_level(LevelFilter::Debug)
            .with_tag("ffi"),
    )
}

fn report_error(error: &dyn std::fmt::Display) {
    static ERRORS: LogThrottle = LogThrottle::new(ERROR_INTERVAL);
    if let Some(count) = ERRORS.due() {
        // Reporting through log::error! here would call this writer recursively.
        logcat().log(
            &Record::builder()
                .level(Level::Error)
                .args(format_args!(
                    "Android file logging failed: {error} (x{count})"
                ))
                .build(),
        );
    }
}
