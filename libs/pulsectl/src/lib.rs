/// `pulsectl` is a high level wrapper around the PulseAudio bindings supplied by
/// `libpulse_binding`. It provides simple access to sinks, inputs, sources and outputs allowing
/// one to write audio control programs with ease.
///
/// ## Quick Example
///
/// The following example demonstrates listing all of the playback devices currently connected
///
/// See examples/change_device_vol.rs for a more complete example
/// ```no_run
/// extern crate pulsectl;
///
/// use std::io;
///
/// use pulsectl::controllers::SinkController;
/// use pulsectl::controllers::DeviceControl;
/// fn main() {
///     // create handler that calls functions on playback devices and apps
///     let mut handler = SinkController::create().unwrap();
///     let devices = handler
///         .list_devices()
///        .expect("Could not get list of playback devices");
///
///     println!("Playback Devices");
///     for dev in devices.clone() {
///         println!(
///             "[{}] {}, Volume: {}",
///             dev.index,
///             dev.description.as_ref().unwrap(),
///             dev.volume.print()
///         );
///     }
/// }
/// ```
extern crate libpulse_binding as pulse;

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use pulse::{
    context::{introspect, Context},
    mainloop::standard::{IterateResult, Mainloop},
    operation::{Operation, State},
    proplist::Proplist,
};

use crate::errors::{PulseCtlError, PulseCtlErrorType::*};

pub mod controllers;
mod errors;

pub struct Handler {
    pub mainloop: Rc<RefCell<Mainloop>>,
    pub context: Rc<RefCell<Context>>,
    pub introspect: introspect::Introspector,
}

fn connect_error(err: &str) -> PulseCtlError {
    PulseCtlError::new(ConnectError, err)
}

impl Handler {
    pub fn connect(name: &str) -> Result<Handler, PulseCtlError> {
        let mut proplist = Proplist::new().unwrap();
        proplist
            .set_str(pulse::proplist::properties::APPLICATION_NAME, name)
            .unwrap();

        let mainloop;
        if let Some(m) = Mainloop::new() {
            mainloop = Rc::new(RefCell::new(m));
        } else {
            return Err(connect_error("Failed to create mainloop"));
        }

        let context;
        if let Some(c) =
            Context::new_with_proplist(mainloop.borrow().deref(), "MainConn", &proplist)
        {
            context = Rc::new(RefCell::new(c));
        } else {
            return Err(connect_error("Failed to create new context"));
        }

        context
            .borrow_mut()
            .connect(None, pulse::context::flags::NOFLAGS, None)
            .map_err(|_| connect_error("Failed to connect context"))?;

        loop {
            match mainloop.borrow_mut().iterate(false) {
                IterateResult::Err(e) => {
                    eprintln!("iterate state was not success, quitting...");
                    return Err(e.into());
                }
                IterateResult::Success(_) => {}
                IterateResult::Quit(_) => {
                    eprintln!("iterate state was not success, quitting...");
                    return Err(PulseCtlError::new(
                        ConnectError,
                        "Iterate state quit without an error",
                    ));
                }
            }

            match context.borrow().get_state() {
                pulse::context::State::Ready => break,
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    eprintln!("context state failed/terminated, quitting...");
                    return Err(PulseCtlError::new(
                        ConnectError,
                        "Context state failed/terminated without an error",
                    ));
                }
                _ => {}
            }
        }

        let introspect = context.borrow_mut().introspect();
        Ok(Handler {
            mainloop,
            context,
            introspect,
        })
    }

    // loop until the passed operation is completed
    pub fn wait_for_operation<G: ?Sized>(
        &mut self,
        op: Operation<G>,
    ) -> Result<(), errors::PulseCtlError> {
        loop {
            match self.mainloop.borrow_mut().iterate(false) {
                IterateResult::Err(e) => return Err(e.into()),
                IterateResult::Success(_) => {}
                IterateResult::Quit(_) => {
                    return Err(PulseCtlError::new(
                        OperationError,
                        "Iterate state quit without an error",
                    ));
                }
            }
            match op.get_state() {
                State::Done => {
                    break;
                }
                State::Running => {}
                State::Cancelled => {
                    return Err(PulseCtlError::new(
                        OperationError,
                        "Operation cancelled without an error",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Drop for Handler {
    fn drop(&mut self) {
        self.context.borrow_mut().disconnect();
        self.mainloop.borrow_mut().quit(pulse::def::Retval(0));
    }
}
