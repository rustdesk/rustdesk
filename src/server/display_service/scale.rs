use base::message_proto::{DisplayScaleRequest, Message, Misc};
use hbb_common::{bail, log, tokio, ResultType};
use std::sync::atomic::{AtomicBool, Ordering};

static BUSY: AtomicBool = AtomicBool::new(false);

struct Operation;
impl Drop for Operation {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

pub(in crate::server) async fn request(request: DisplayScaleRequest, allowed: bool) -> Message {
    let request_id = request.request_id.clone();
    let result: ResultType<crate::platform::display_scale::State> = async {
        if !allowed {
            bail!("No permission to change display settings.");
        }
        if request_id.is_empty()
            || request_id.len() > 64
            || request.token.len() > 64
            || request.expected_identity.len() > 64
            || (request.percent != 0.0 && !request.expected_identity.is_empty())
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
        tokio::task::spawn_blocking(move || {
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
                crate::platform::display_scale::configure(&display, request.percent, &request.token)
            }
        })
        .await?
    }
    .await;
    let response = match result {
        Ok(state) => serde_json::json!({"request_id": request_id, "state": state}),
        Err(error) => {
            log::debug!("Display scaling: {error}");
            let mut response =
                serde_json::json!({"request_id": request_id, "error": error.to_string()});
            if error.is::<crate::platform::display_scale::Unsupported>() {
                response["code"] = serde_json::json!("unsupported");
            } else if error.is::<crate::platform::display_scale::SnapshotChanged>() {
                response["code"] = serde_json::json!("snapshot_changed");
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
