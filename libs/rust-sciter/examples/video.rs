#![allow(unused_variables, unused_must_use)]

extern crate sciter;

use sciter::dom::event::*;
use sciter::{Element, HELEMENT};

use sciter::video::{fragmented_video_destination, AssetPtr};

struct VideoGen {
  thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for VideoGen {
  fn drop(&mut self) {
    println!("[video] behavior is destroyed");
  }
}

impl VideoGen {
  fn new() -> Self {
    Self { thread: None }
  }

  fn generation_thread(site: AssetPtr<fragmented_video_destination>) {
    println!("[video] thread is started");

    // our video frame size and its part to update
    const FRAME: (i32, i32) = (1200, 800);
    const UPDATE: (i32, i32) = (256, 32);

    // our frame data (RGBA)
    let figure = [0xFF_FFA500u32; (UPDATE.0 * UPDATE.1) as usize];

    // configure video output
    let mut site = site;
    let ok = site.start_streaming(FRAME, sciter::video::COLOR_SPACE::Rgb32, None);
    println!("[video] initialized: {:?}", ok);

    let mut x = 0;
    let mut xd = 1;
    let mut y = 0;
    let mut yd = 1;
    while site.is_alive() {
      // send an update portion
      let buf: &[u8] = unsafe { std::mem::transmute(figure.as_ref()) };
      site.render_frame_part(buf, (x, y), UPDATE);

      // set the next position
      x += xd;
      y += yd;

      if x == 0 {
        xd = 1;
      } else if x + UPDATE.0 == FRAME.0 {
        xd = -1;
      }
      if y == 0 {
        yd = 1;
      } else if y + UPDATE.1 == FRAME.1 {
        yd = -1;
      }

      // simulate 25 FPS
      std::thread::sleep(std::time::Duration::from_millis(1000 / 25));
    }

    site.stop_streaming();
    println!("[video] thread is finished");
  }
}

impl sciter::EventHandler for VideoGen {
  fn get_subscription(&mut self) -> Option<EVENT_GROUPS> {
    Some(EVENT_GROUPS::HANDLE_BEHAVIOR_EVENT)
  }

  fn detached(&mut self, _root: HELEMENT) {
    println!("[video] <video> element is detached");
    if let Some(h) = self.thread.take() {
      h.join();
    }
  }

  fn on_event(
    &mut self,
    root: HELEMENT,
    source: HELEMENT,
    target: HELEMENT,
    code: BEHAVIOR_EVENTS,
    phase: PHASE_MASK,
    reason: EventReason,
  ) -> bool {
    if phase != PHASE_MASK::BUBBLING {
      return false;
    }

    match code {
      BEHAVIOR_EVENTS::VIDEO_BIND_RQ => {
        let source = Element::from(source);
        println!("[video] {:?} {} ({:?})", code, source, reason);

        if let EventReason::VideoBind(ptr) = reason {
          if ptr.is_null() {
            // first, consume the event to announce us as a video producer.
            return true;
          }

          use sciter::video::*;

          // `VideoBind` comes with a video_destination interface
          let mut site = AssetPtr::from(ptr as *mut video_destination);

          // query a fragmented video destination interface
          if let Ok(fragmented) = AssetPtr::<fragmented_video_destination>::try_from(&mut site) {
            // and use it
            println!("[video] start video thread");

            let tid = ::std::thread::spawn(|| VideoGen::generation_thread(fragmented));
            self.thread = Some(tid);
          }
        }
      }

      BEHAVIOR_EVENTS::VIDEO_INITIALIZED => {
        println!("[video] {:?}", code);
      }

      BEHAVIOR_EVENTS::VIDEO_STARTED => {
        println!("[video] {:?}", code);

        let source = Element::from(source);
	      use sciter::dom::ELEMENT_AREAS;
		    let flags = ELEMENT_AREAS::CONTENT_BOX as u32 | ELEMENT_AREAS::SELF_RELATIVE as u32;
		    let rc = source.get_location(flags).unwrap();
	      println!("[video] start video thread on <{}> which is about {:?} pixels", source, rc.size());
      }

      BEHAVIOR_EVENTS::VIDEO_STOPPED => {
        println!("[video] {:?}", code);
      }

      _ => return false,
    };

    return true;
  }
}

fn main() {
  if cfg!(all(target_os = "windows", target_arch = "x86")) {
    println!("\nerror: Sciter video will not work on Windows x86.");
    println!("error: Consider using a nightly Rust version to enable `abi_thiscall`,");
		println!("error: see https://github.com/rust-lang/rust/issues/42202");
		println!("");
    std::process::exit(126);
	}

	if sciter::version_num() < 0x04_04_02_0E {
		// since 4.4.2.14
		println!("\nerror: `sciter::video` requires SOM support.");
		println!("error: Sciter API was changed in '4.4.2.14'");
		println!("error: Sciter version is '{}' now", sciter::version());
		println!("error: see https://sciter.com/native-code-exposure-to-script/");
		println!("error: and https://sciter.com/developers/for-native-gui-programmers/sciter-object-model/");
		println!("");
		std::process::exit(126);
	}

  let mut frame = sciter::WindowBuilder::main_window()
  	.with_size((750, 750))
  	.create();
  frame.set_title("Video renderer sample");
  frame.register_behavior("video-generator", || Box::new(VideoGen::new()));
  frame.load_html(include_bytes!("video.htm"), Some("example://video.htm"));
  frame.run_app();
}
