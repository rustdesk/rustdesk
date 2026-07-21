use hbb_common::{
    config::Config,
    log,
    sysinfo::{Pid, ProcessRefreshKind, System},
};
use std::{sync::Once, thread, time::Duration};

const THRESHOLD_OPTION: &str = "rdh-memory-restart-threshold-mib";
const DEFAULT_THRESHOLD_MIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;
const INITIAL_GRACE: Duration = Duration::from_secs(10 * 60);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(5 * 60);
const FINAL_IDLE_GRACE: Duration = Duration::from_secs(30);
const REQUIRED_IDLE_OVER_LIMIT_SAMPLES: u8 = 2;
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
        "RDH memory watchdog enabled: threshold={} MiB, idle_samples={}, sample_interval={}s",
        threshold_mib,
        REQUIRED_IDLE_OVER_LIMIT_SAMPLES,
        SAMPLE_INTERVAL.as_secs()
    );
    Some(threshold_bytes)
}

fn run(threshold_bytes: u64) {
    thread::sleep(INITIAL_GRACE);

    let current_pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    let mut idle_over_limit_samples = 0;

    loop {
        let Some(rss_bytes) = current_rss_bytes(&mut system, current_pid) else {
            log::error!("RDH memory watchdog could not read the --server RSS");
            idle_over_limit_samples = 0;
            thread::sleep(SAMPLE_INTERVAL);
            continue;
        };
        let active_connections = crate::Connection::alive_conns().len();
        idle_over_limit_samples = next_idle_over_limit_samples(
            rss_bytes,
            threshold_bytes,
            active_connections,
            idle_over_limit_samples,
        );

        if rss_bytes >= threshold_bytes {
            if active_connections > 0 {
                log::warn!(
                    "RDH memory watchdog deferred restart: rss={} MiB, active_connections={}",
                    rss_bytes / MIB,
                    active_connections
                );
            } else {
                log::warn!(
                    "RDH memory watchdog observed idle over-limit server: rss={} MiB, sample={}/{}",
                    rss_bytes / MIB,
                    idle_over_limit_samples,
                    REQUIRED_IDLE_OVER_LIMIT_SAMPLES
                );
            }
        }

        if idle_over_limit_samples >= REQUIRED_IDLE_OVER_LIMIT_SAMPLES {
            thread::sleep(FINAL_IDLE_GRACE);
            let final_rss_bytes = current_rss_bytes(&mut system, current_pid);
            let final_active_connections = crate::Connection::alive_conns().len();
            if final_rss_bytes.is_some_and(|rss| rss >= threshold_bytes)
                && final_active_connections == 0
            {
                log::error!(
                    "RDH memory watchdog restarting idle --server for high RSS; launchd will relaunch it"
                );
                std::process::exit(RESTART_EXIT_CODE);
            }

            log::info!(
                "RDH memory watchdog cancelled restart during final gate: rss_mib={:?}, active_connections={}",
                final_rss_bytes.map(|rss| rss / MIB),
                final_active_connections
            );
            idle_over_limit_samples = 0;
        }

        thread::sleep(SAMPLE_INTERVAL);
    }
}

fn current_rss_bytes(system: &mut System, pid: Pid) -> Option<u64> {
    system.refresh_process_specifics(pid, ProcessRefreshKind::new());
    system.process(pid).map(|process| process.memory())
}

fn next_idle_over_limit_samples(
    rss_bytes: u64,
    threshold_bytes: u64,
    active_connections: usize,
    previous_samples: u8,
) -> u8 {
    if rss_bytes < threshold_bytes || active_connections > 0 {
        return 0;
    }

    previous_samples.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_consecutive_idle_over_limit_samples() {
        let first = next_idle_over_limit_samples(1024, 1000, 0, 0);
        let second = next_idle_over_limit_samples(1024, 1000, 0, first);

        assert_eq!(first, 1);
        assert_eq!(second, REQUIRED_IDLE_OVER_LIMIT_SAMPLES);
    }

    #[test]
    fn active_connection_resets_over_limit_samples() {
        assert_eq!(next_idle_over_limit_samples(1024, 1000, 1, 1), 0);
    }

    #[test]
    fn memory_recovery_resets_over_limit_samples() {
        assert_eq!(next_idle_over_limit_samples(999, 1000, 0, 1), 0);
    }
}
