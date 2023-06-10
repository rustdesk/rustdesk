/// Url handler based on dbus
///
/// Note:
/// On linux, we use dbus to communicate multiple rustdesk process.
/// [Flutter]: handle uni links for linux
use dbus::blocking::Connection;
use dbus_crossroads::{Crossroads, IfaceBuilder};
use hbb_common::log;
#[cfg(feature = "flutter")]
use std::collections::HashMap;
use std::{error::Error, fmt, time::Duration};

const DBUS_NAME: &str = "org.rustdesk.rustdesk";
const DBUS_PREFIX: &str = "/dbus";
const DBUS_METHOD_NEW_CONNECTION: &str = "NewConnection";
const DBUS_METHOD_NEW_CONNECTION_ID: &str = "id";
const DBUS_METHOD_RETURN: &str = "ret";
const DBUS_METHOD_RETURN_SUCCESS: &str = "ok";
const DBUS_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct DbusError(String);

impl fmt::Display for DbusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RustDesk DBus Error: {}", self.0)
    }
}

impl Error for DbusError {}

/// invoke new connection from dbus
///
/// [Tips]:
/// How to test by CLI:
/// - use dbus-send command:
/// `dbus-send --session --print-reply --dest=org.rustdesk.rustdesk /dbus org.rustdesk.rustdesk.NewConnection string:'PEER_ID'`
pub fn invoke_new_connection(uni_links: String) -> Result<(), Box<dyn Error>> {
    log::info!("Starting dbus service for uni");
    let conn = Connection::new_session()?;
    let proxy = conn.with_proxy(DBUS_NAME, DBUS_PREFIX, DBUS_TIMEOUT);
    let (ret,): (String,) =
        proxy.method_call(DBUS_NAME, DBUS_METHOD_NEW_CONNECTION, (uni_links,))?;
    if ret != DBUS_METHOD_RETURN_SUCCESS {
        log::error!("error on call new connection to dbus server");
        return Err(Box::new(DbusError("not success".to_string())));
    }
    Ok(())
}

/// start dbus server
///
/// [Blocking]:
/// The function will block current thread to serve dbus server.
/// So it's suitable to spawn a new thread dedicated to dbus server.
pub fn start_dbus_server() -> Result<(), Box<dyn Error>> {
    let conn: Connection = Connection::new_session()?;
    let _ = conn.request_name(DBUS_NAME, false, true, false)?;
    let mut cr = Crossroads::new();
    let token = cr.register(DBUS_NAME, handle_client_message);
    cr.insert(DBUS_PREFIX, &[token], ());
    cr.serve(&conn)?;
    Ok(())
}

fn handle_client_message(builder: &mut IfaceBuilder<()>) {
    // register new connection dbus
    builder.method(
        DBUS_METHOD_NEW_CONNECTION,
        (DBUS_METHOD_NEW_CONNECTION_ID,),
        (DBUS_METHOD_RETURN,),
        move |_, _, (_uni_links,): (String,)| {
            #[cfg(feature = "flutter")]
            {
                use crate::flutter;
                let data = HashMap::from([
                    ("name", "on_url_scheme_received"),
                    ("url", _uni_links.as_str()),
                ]);
                let event = serde_json::ser::to_string(&data).unwrap_or("".to_string());
                match crate::flutter::push_global_event(flutter::APP_TYPE_MAIN, event) {
                    None => log::error!("failed to find main event stream"),
                    Some(false) => {
                        log::error!("failed to add dbus message to flutter global dbus stream.")
                    }
                    Some(true) => {}
                }
            }
            return Ok((DBUS_METHOD_RETURN_SUCCESS.to_string(),));
        },
    );
}
