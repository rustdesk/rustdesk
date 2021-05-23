//! Download http content (Go sciter example port).
#![allow(dead_code)]

extern crate sciter;

use sciter::dom::HELEMENT;
use sciter::host;
use sciter::utf;
use std::rc::{Rc, Weak};

struct Handler {
	host: Weak<sciter::Host>,
}

impl sciter::EventHandler for Handler {
  fn document_complete(&mut self, _root: HELEMENT, _target: HELEMENT) {
    if let Some(host) = self.host.upgrade() {
      // eval script inside the document to receive a "user@machine" string.
      let result = host.eval_script("[Sciter.userName(), Sciter.machineName(true)].join(`@`)");
      match result {
        Ok(name) => {
          println!("running on {}", name);
        }
        Err(e) => {
          println!("error! {}", e.as_string().unwrap_or("?".to_string()));
        }
      }
    }
  }
}

impl sciter::HostHandler for Handler {
	fn on_data_loaded(&mut self, pnm: &host::SCN_DATA_LOADED) {
		println!("data loaded, uri: `{}`, {} bytes.", utf::w2s(pnm.uri), pnm.dataSize);
	}

	fn on_attach_behavior(&mut self, pnm: &mut host::SCN_ATTACH_BEHAVIOR) -> bool {
		let el = sciter::Element::from(pnm.element);
		let name = utf::u2s(pnm.name);
		println!("{}: behavior {}", el, name);
		false
	}
}

impl Drop for Handler {
	fn drop(&mut self) {
		// called 2 times because it is created 2 times
		println!("Good bye, window");
	}
}

fn main() {
  let mut frame = sciter::WindowBuilder::main_window().with_size((1024, 768)).create();

  // Can't use something like `frame.sciter_handler(Rc::new(handler))` yet.
  let handler = Handler {
    host: Rc::downgrade(&frame.get_host()),
  };
	frame.sciter_handler(handler);

  let handler = Handler {
    host: Rc::downgrade(&frame.get_host()),
  };
  frame.event_handler(handler);

	frame.set_title("Download sample");
	frame.load_file("http://httpbin.org/html");
	frame.run_app();
}
