use base::fs::android;
use hbb_common::{
    config::Config,
    flexi_logger::{
        opt_format,
        writers::{FileLogWriter, LogWriter},
        Age, Cleanup, Criterion, DeferredNow, FileSpec, FlexiLoggerError, Logger, Naming,
    },
    log::{Level, LevelFilter, Log, Record},
    log_throttle::LogThrottle,
};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Once,
    },
    time::Duration,
};

const ERROR_INTERVAL: Duration = Duration::from_secs(60);
const DEFAULT_LOG_MAX_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_LOG_FILE_COUNT: usize = 90;

// The first bridge request initializes Kotlin's persisted log-folder selection.
static PUBLIC_LOGS_ENABLED: AtomicBool = AtomicBool::new(true);

#[no_mangle]
pub extern "system" fn Java_com_carriez_flutter_1hbb_DocumentLogs_setEnabled(
    _env: jni::JNIEnv,
    _instance: jni::objects::JObject,
    enabled: jni::sys::jboolean,
) {
    PUBLIC_LOGS_ENABLED.store(enabled != jni::sys::JNI_FALSE, Ordering::Relaxed);
}

pub fn initialize() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let result = Logger::try_with_env_or_str("debug,reqwest=warn,hyper_util=warn,rustls=warn,webrtc-sctp=warn,webrtc=warn,webrtc_ice::agent::agent_internal=error,webrtc::peer_connection=error")
            .and_then(|logger| {
                let default_writer = FileLogWriter::builder(FileSpec::default().directory(Config::log_path()))
                    .format(opt_format)
                    .rotate(
                        Criterion::AgeOrSize(Age::Day, DEFAULT_LOG_MAX_BYTES),
                        Naming::Timestamps,
                        Cleanup::KeepLogFiles(DEFAULT_LOG_FILE_COUNT),
                    )
                    .try_build();
                logger.log_to_writer(Box::new(DocumentWriter { default_writer })).start()
            });
        if let Err(error) = result {
            report_error(&error);
        }
    });
}

struct DocumentWriter {
    default_writer: Result<FileLogWriter, FlexiLoggerError>,
}

impl DocumentWriter {
    fn write_default(&self, now: &mut DeferredNow, record: &Record) -> io::Result<()> {
        self.default_writer
            .as_ref()
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?
            .write(now, record)
    }
}

impl LogWriter for DocumentWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> io::Result<()> {
        // JNI logs while detaching threads; calling back into JNI there is unsafe.
        if record.target() == "jni" || record.target().starts_with("jni::") {
            logcat().log(record);
            return Ok(());
        }
        if !PUBLIC_LOGS_ENABLED.load(Ordering::Relaxed) {
            return self.write_default(now, record);
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
            Ok(value) if value.as_bool() == Some(false) => self.write_default(now, record),
            result => {
                let error = match result {
                    Err(error) => error.to_string(),
                    _ => "Invalid document logger response".to_owned(),
                };
                let result = self.write_default(now, record).map_err(|default_error| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "Public log directory: {error}; default log directory: {default_error}"
                        ),
                    )
                });
                match &result {
                    Ok(()) => report_error(&format!(
                        "{error}; writing to the original default log directory"
                    )),
                    Err(error) => report_error(error),
                }
                result
            }
        }
    }

    fn flush(&self) -> io::Result<()> {
        self.default_writer
            .as_ref()
            .map_or(Ok(()), |writer| writer.flush())
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
