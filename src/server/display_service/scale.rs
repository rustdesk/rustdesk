use base::{
    config::keys,
    message_proto::{DisplayScaleRequest, Message, Misc},
};
use hbb_common::{bail, config::Config, log, tokio, ResultType};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

static BUSY: AtomicBool = AtomicBool::new(false);
static TRACKING: OnceLock<(Mutex<Tracking>, Condvar)> = OnceLock::new();

#[derive(Clone, Copy)]
struct OriginalScale {
    resolution: (u32, u32),
    percent: f64,
}

#[derive(Default)]
struct Tracking {
    pending: usize,
    restoring: bool,
    originals: HashMap<String, OriginalScale>,
    next_resolution: u64,
    serving_resolution: u64,
}

fn tracking() -> &'static (Mutex<Tracking>, Condvar) {
    TRACKING.get_or_init(|| (Mutex::new(Tracking::default()), Condvar::new()))
}

struct Pending;

impl Drop for Pending {
    fn drop(&mut self) {
        let (lock, ready) = tracking();
        let mut state = lock.lock().unwrap();
        state.pending -= 1;
        ready.notify_all();
    }
}

fn reserve() -> ResultType<Pending> {
    let mut state = tracking().0.lock().unwrap();
    if state.restoring {
        bail!("Display settings are busy. Try again.");
    }
    state.pending += 1;
    Ok(Pending)
}

pub(in crate::server) struct ResolutionReservation {
    pub(in crate::server) capture: bool,
    _pending: Option<Pending>,
}

struct ResolutionTurn;

impl Drop for ResolutionTurn {
    fn drop(&mut self) {
        let (lock, ready) = tracking();
        let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.serving_resolution = state.serving_resolution.wrapping_add(1);
        ready.notify_all();
    }
}

pub(in crate::server) fn reserve_resolution() -> Option<ResolutionReservation> {
    let capture = Config::get_bool_option(keys::OPTION_ALLOW_DISPLAY_SCALING);
    let mut state = tracking().0.lock().unwrap();
    if state.restoring {
        log::trace!("Resolution change deferred: display settings are busy");
        return None;
    }
    let pending = if capture || state.pending != 0 || !state.originals.is_empty() {
        state.pending += 1;
        Some(Pending)
    } else {
        None
    };
    Some(ResolutionReservation {
        capture,
        _pending: pending,
    })
}

fn remember(state: &crate::platform::display_scale::State) {
    let mut tracking = tracking().0.lock().unwrap();
    tracking
        .originals
        .entry(state.identity.clone())
        .or_insert(OriginalScale {
            resolution: state.resolution,
            percent: state.percent,
        });
}

pub(in crate::server) fn change_resolution_after_capture(
    operation: ResolutionReservation,
    display: &scrap::Display,
    name: String,
    original: (i32, i32),
    requested: (i32, i32),
) -> tokio::task::JoinHandle<()> {
    let captured = crate::platform::display_scale::Display {
        name: name.clone(),
        origin: display.origin(),
        size: (display.width(), display.height()),
    };
    let ticket = {
        let mut state = tracking().0.lock().unwrap();
        let ticket = state.next_resolution;
        state.next_resolution = state.next_resolution.wrapping_add(1);
        ticket
    };
    tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let (lock, ready) = tracking();
        let mut state = lock.lock().unwrap();
        while state.serving_resolution != ticket {
            state = ready.wait(state).unwrap();
        }
        drop(state);
        let _turn = ResolutionTurn;
        match crate::platform::display_scale::configure(&captured, 0.0, "") {
            Ok(state) => remember(&state),
            Err(error) => log::trace!("Could not record original display scale: {error}"),
        }
        super::set_last_changed_resolution(&name, original, requested);
        if let Err(error) =
            crate::platform::change_resolution(&name, requested.0 as _, requested.1 as _)
        {
            log::error!(
                "Failed to change resolution '{}' to ({},{}): {:?}",
                name,
                requested.0,
                requested.1,
                error
            );
        }
    })
}

#[derive(Debug)]
struct Disabled;

impl std::fmt::Display for Disabled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Display scaling is disabled on the controlled machine.")
    }
}

impl std::error::Error for Disabled {}

fn restore_original(identity: &str, original: OriginalScale) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let displays = scrap::Display::all().unwrap_or_else(|error| {
            log::trace!("Could not enumerate displays for scale restoration: {error}");
            Vec::new()
        });
        for info in displays {
            if !info.is_online() || info.width() == 0 || info.height() == 0 {
                continue;
            }
            let display = crate::platform::display_scale::Display {
                name: info.name(),
                origin: info.origin(),
                size: (info.width(), info.height()),
            };
            let Ok(current) =
                crate::platform::display_scale::read_confirmed(Some(&display), identity)
            else {
                continue;
            };
            if current.resolution != original.resolution {
                continue;
            }
            if (current.percent - original.percent).abs() < 0.000001 {
                return;
            }
            match crate::platform::display_scale::restore_original(
                &display,
                original.percent,
                &current.token,
            ) {
                Ok(_) => return,
                Err(error) => log::trace!("Could not yet restore display scale: {error}"),
            }
        }
        if Instant::now() >= deadline {
            log::warn!("Could not restore original scale for display {identity}");
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct RestoreGuard;

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        let (lock, ready) = tracking();
        let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.restoring = false;
        ready.notify_all();
    }
}

fn restore_after_pending() {
    let _restore_guard = RestoreGuard;
    let (lock, ready) = tracking();
    let mut state = lock.lock().unwrap();
    while state.pending != 0 {
        state = ready.wait(state).unwrap();
    }
    let originals = std::mem::take(&mut state.originals);
    drop(state);

    super::restore_resolutions();
    for (identity, original) in originals {
        restore_original(&identity, original);
    }
    #[cfg(any(windows, target_os = "linux"))]
    let no_remote = !crate::server::AUTHED_CONNS
        .lock()
        .unwrap()
        .iter()
        .any(|connection| connection.conn_type == crate::server::AuthConnType::Remote);
    #[cfg(windows)]
    if no_remote {
        if let Err(error) = crate::virtual_display_manager::reset_all() {
            log::error!("Could not reset virtual displays after scale restoration: {error}");
        }
    }
    #[cfg(target_os = "linux")]
    if no_remote {
        scrap::wayland::pipewire::try_close_session();
    }
}

pub(in crate::server) fn restore_at_disconnect() -> bool {
    let mut state = tracking().0.lock().unwrap();
    if state.restoring {
        return true;
    }
    if state.pending == 0 && state.originals.is_empty() {
        return false;
    }
    state.restoring = true;
    drop(state);
    if let Err(error) = std::thread::Builder::new()
        .name("restore-display-scale".into())
        .spawn(restore_after_pending)
    {
        log::error!("Could not start display scale restoration: {error}");
        restore_after_pending();
    }
    true
}

struct Operation;
impl Drop for Operation {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

pub(in crate::server) fn request(
    request: DisplayScaleRequest,
    allowed: bool,
) -> impl std::future::Future<Output = Message> {
    let preflight: ResultType<(Operation, Option<Pending>, bool, bool)> = (|| {
        if !allowed {
            bail!("No permission to change display settings.");
        }
        if request.request_id.is_empty()
            || request.request_id.len() > 64
            || request.token.len() > 64
            || request.expected_identity.len() > 64
            || (request.percent != 0.0 && !request.expected_identity.is_empty())
            || (request.percent != 0.0 && request.token.is_empty())
            || (request.percent != 0.0
                && request.percent != -1.0
                && !(50.0..=500.0).contains(&request.percent))
        {
            bail!("Invalid display scaling request.");
        }
        if BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            bail!("Display settings are busy. Try again.");
        }
        let operation = Operation;
        let enabled = Config::get_bool_option(keys::OPTION_ALLOW_DISPLAY_SCALING);
        let tracked_original =
            request.percent == -1.0 && !tracking().0.lock().unwrap().originals.is_empty();
        let pending = if enabled && (request.percent > 0.0 || tracked_original) {
            Some(reserve()?)
        } else {
            None
        };
        Ok((operation, pending, enabled, tracked_original))
    })();
    async move {
        let request_id = request.request_id.clone();
        let result: ResultType<crate::platform::display_scale::State> = async {
            let (operation, pending, enabled, tracked_original) = preflight?;
            tokio::task::spawn_blocking(move || {
                let _pending = pending;
                let _operation = operation;
                let Ok(index) = usize::try_from(request.display) else {
                    bail!(crate::platform::display_scale::STALE);
                };
                let display = super::get_display_info(index)
                    .filter(|d| d.online && d.width > 0 && d.height > 0)
                    .map(|display| crate::platform::display_scale::Display {
                        name: display.name,
                        origin: (display.x, display.y),
                        size: (display.width as usize, display.height as usize),
                    });
                if !request.expected_identity.is_empty() {
                    crate::platform::display_scale::read_confirmed(
                        display.as_ref(),
                        &request.expected_identity,
                    )
                } else {
                    let Some(display) = display else {
                        bail!(crate::platform::display_scale::STALE);
                    };
                    if request.percent == -1.0 {
                        let current = crate::platform::display_scale::configure(&display, 0.0, "")?;
                        if request.token.is_empty() || request.token != current.token {
                            bail!(crate::platform::display_scale::STALE);
                        }
                        let original = if tracked_original || !enabled {
                            tracking()
                                .0
                                .lock()
                                .unwrap()
                                .originals
                                .get(&current.identity)
                                .copied()
                        } else {
                            None
                        };
                        if let Some(original) = original {
                            if current.resolution != original.resolution {
                                bail!(crate::platform::display_scale::STALE);
                            }
                            if (current.percent - original.percent).abs() < 0.000001 {
                                Ok(current)
                            } else {
                                if !enabled
                                    || !Config::get_bool_option(keys::OPTION_ALLOW_DISPLAY_SCALING)
                                {
                                    bail!(Disabled);
                                }
                                crate::platform::display_scale::restore_original(
                                    &display,
                                    original.percent,
                                    &current.token,
                                )
                            }
                        } else {
                            Ok(current)
                        }
                    } else if request.percent == 0.0 {
                        crate::platform::display_scale::configure(&display, 0.0, "")
                    } else {
                        if !enabled || !Config::get_bool_option(keys::OPTION_ALLOW_DISPLAY_SCALING)
                        {
                            bail!(Disabled);
                        }
                        let original =
                            crate::platform::display_scale::configure(&display, 0.0, "")?;
                        crate::platform::display_scale::validate(
                            &original,
                            request.percent,
                            &request.token,
                        )?;
                        if (original.percent - request.percent).abs() >= 0.000001 {
                            remember(&original);
                        }
                        crate::platform::display_scale::configure(
                            &display,
                            request.percent,
                            &request.token,
                        )
                    }
                }
            })
            .await?
        }
        .await;
        let response = match result {
            Ok(state) => serde_json::json!({"request_id": request_id, "state": state}),
            Err(error) => {
                log::trace!("Display scaling: {error}");
                let mut response =
                    serde_json::json!({"request_id": request_id, "error": error.to_string()});
                if error.is::<crate::platform::display_scale::Unsupported>() {
                    response["code"] = serde_json::json!("unsupported");
                } else if error.is::<crate::platform::display_scale::SnapshotChanged>() {
                    response["code"] = serde_json::json!("snapshot_changed");
                } else if error.is::<Disabled>() {
                    response["code"] = serde_json::json!("disabled");
                }
                response
            }
        };
        let mut misc = Misc::new();
        misc.set_display_scale_response(response.to_string());
        let mut message = Message::new();
        message.set_misc(misc);
        message
    }
}
