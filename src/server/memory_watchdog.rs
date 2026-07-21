use chrono::{Local, Timelike};
use hbb_common::{
    config::Config,
    log,
    sysinfo::{Pid, ProcessRefreshKind, System},
};
use std::{sync::Once, thread, time::Duration};

const THRESHOLD_OPTION: &str = "rdh-memory-restart-threshold-mib";
const DEFAULT_THRESHOLD_MIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;
const DAILY_CHECK_HOUR: u32 = 6;
const UNATTENDED_WINDOW_START_HOUR: u32 = 0;
const UNATTENDED_WINDOW_END_HOUR: u32 = 7;
const SECONDS_PER_HOUR: u64 = 60 * 60;
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;
const RESTART_EXIT_CODE: i32 = 75;

static START: Once = Once::new();

pub fn start() {
    START.call_once(|| {
        if !is_launchd_supervised() {
            log::warn!("RDH memory watchdog disabled: --server is not launchd-supervised");
            return;
        }

        let Some(threshold_bytes) = configured_threshold_bytes() else {
            return;
        };

        if let Err(err) = thread::Builder::new()
            .name("rdh-memory-watchdog".to_owned())
            .spawn(move || run(threshold_bytes))
        {
            log::error!("Failed to start RDH memory watchdog: {err}");
        }
    });
}

fn is_launchd_supervised() -> bool {
    let expected_service_name = format!("{}_server", crate::get_full_name());
    std::env::var("XPC_SERVICE_NAME").ok().as_deref() == Some(expected_service_name.as_str())
}

fn configured_threshold_bytes() -> Option<u64> {
    let raw_value = Config::get_option(THRESHOLD_OPTION);
    let threshold_mib = if raw_value.trim().is_empty() {
        DEFAULT_THRESHOLD_MIB
    } else {
        match raw_value.trim().parse::<u64>() {
            Ok(value) => value,
            Err(err) => {
                log::error!(
                    "RDH memory watchdog disabled: invalid {THRESHOLD_OPTION}={raw_value:?}: {err}"
                );
                return None;
            }
        }
    };

    if threshold_mib == 0 {
        log::info!("RDH memory watchdog disabled by {THRESHOLD_OPTION}=0");
        return None;
    }

    let Some(threshold_bytes) = threshold_mib.checked_mul(MIB) else {
        log::error!(
            "RDH memory watchdog disabled: {THRESHOLD_OPTION}={threshold_mib} is too large"
        );
        return None;
    };

    log::info!(
        "RDH memory watchdog enabled: threshold={} MiB, daily_check={:02}:00, unattended_window={:02}:00-{:02}:00",
        threshold_mib,
        DAILY_CHECK_HOUR,
        UNATTENDED_WINDOW_START_HOUR,
        UNATTENDED_WINDOW_END_HOUR
    );
    Some(threshold_bytes)
}

fn run(threshold_bytes: u64) {
    let current_pid = Pid::from_u32(std::process::id());
    let mut system = System::new();

    loop {
        let now = Local::now();
        thread::sleep(Duration::from_secs(seconds_until_next_check(
            now.num_seconds_from_midnight(),
        )));

        let check_time = Local::now();
        if !is_unattended_window(check_time.hour()) {
            log::warn!(
                "RDH memory watchdog skipped delayed daily check outside unattended window: hour={}",
                check_time.hour()
            );
            continue;
        }

        let Some(rss_bytes) = current_rss_bytes(&mut system, current_pid) else {
            log::error!("RDH memory watchdog could not read the --server RSS");
            continue;
        };

        if rss_bytes >= threshold_bytes {
            log::error!(
                "RDH memory watchdog restarting over-limit --server during unattended window: rss={} MiB; active connections intentionally ignored; launchd will relaunch it",
                rss_bytes / MIB
            );
            std::process::exit(RESTART_EXIT_CODE);
        }

        log::info!(
            "RDH memory watchdog daily check passed: rss={} MiB, threshold={} MiB",
            rss_bytes / MIB,
            threshold_bytes / MIB
        );
    }
}

fn seconds_until_next_check(now_seconds: u32) -> u64 {
    let now_seconds = u64::from(now_seconds);
    let scheduled_seconds = u64::from(DAILY_CHECK_HOUR) * SECONDS_PER_HOUR;
    if now_seconds < scheduled_seconds {
        scheduled_seconds - now_seconds
    } else {
        SECONDS_PER_DAY - now_seconds + scheduled_seconds
    }
}

fn is_unattended_window(hour: u32) -> bool {
    (UNATTENDED_WINDOW_START_HOUR..UNATTENDED_WINDOW_END_HOUR).contains(&hour)
}

fn current_rss_bytes(system: &mut System, pid: Pid) -> Option<u64> {
    system.refresh_process_specifics(pid, ProcessRefreshKind::new());
    system.process(pid).map(|process| process.memory())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_next_daily_check() {
        assert_eq!(seconds_until_next_check(5 * 60 * 60), 60 * 60);
        assert_eq!(
            seconds_until_next_check(6 * 60 * 60 + 60),
            23 * 60 * 60 + 59 * 60
        );
    }

    #[test]
    fn unattended_window_is_midnight_until_seven() {
        assert!(is_unattended_window(0));
        assert!(is_unattended_window(6));
        assert!(!is_unattended_window(7));
    }
}
