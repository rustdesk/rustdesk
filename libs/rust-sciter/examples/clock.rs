//#![windows_subsystem = "windows"]

#[macro_use]
extern crate sciter;

use sciter::dom::event::{MethodParams, DRAW_EVENTS, EVENT_GROUPS};
use sciter::dom::{Element, HELEMENT};
use sciter::graphics::{self, rgb, Graphics, HGFX};
use sciter::types::RECT;
use sciter::Value;

// 24:60:60, will be drawn as analog clock
type Time = [u8; 3usize];

/// Clock native behavior.
///
/// ## Behavior-specific HTML attributes:
///
/// * `utc="integer"` - time zone offset, positive or negative.
/// * `frozen` - time is not updated automtically.
///
/// ## Value
///
/// *read/write* Current time value in `HH::MM::SS` or `[HH, MM, SS]` form.
///
/// ## Events
///
/// N/A - this element does not generate any specific events.
///
#[derive(Default)]
struct Clock {
  element: Option<Element>,
  now: Time,
  gmt: i8,
  is_frozen: bool,
}

impl sciter::EventHandler for Clock {
  /// Claim what kind of events we want to receive.
  fn get_subscription(&mut self) -> Option<EVENT_GROUPS> {
    // we need timer and draw events
    // also behavior method calls
    Some(EVENT_GROUPS::HANDLE_TIMER
      | EVENT_GROUPS::HANDLE_DRAW
      | EVENT_GROUPS::HANDLE_METHOD_CALL
    )
  }

  /// Our element is constructed. But scripts in HTML are not loaded yet.
  fn attached(&mut self, root: HELEMENT) {
    self.element = Some(Element::from(root));
    let me = self.element.as_ref().unwrap();

    // get attributes to initialize our clock
    if let Some(attr) = me.get_attribute("utc") {
      if let Ok(v) = attr.parse::<i8>() {
        self.gmt = v;
      }
    }

    // we don't update frozen clocks
    if let Some(_attr) = me.get_attribute("frozen") {
      self.is_frozen = true;
    }

    // timer to redraw our clock
    if !self.is_frozen {
      me.start_timer(300, 1).expect("Can't set timer");
    }
  }

  /// Our behavior methods.
  fn on_method_call(&mut self, _root: HELEMENT, params: MethodParams) -> bool {
    match params {
      MethodParams::GetValue(retval) => {
        // engine wants out current value (e.g. `current = element.value`)
        let v: Value = self.now.iter().map(|v| i32::from(*v)).collect();
        println!("return current time as {:?}", v);
        *retval = v;
      }

      MethodParams::SetValue(v) => {
        // engine sets our value (e.g. `element.value = new`)
        println!("set current time from {:?}", v);

        // "10:20:30"
        if v.is_string() {
          let s = v.as_string().unwrap();
          let t = s.split(':').take(3).map(|n| n.parse::<u8>());
          let mut new_time = Time::default();
          for (i, n) in t.enumerate() {
            if let Err(_) = n {
              eprintln!("clock::set_value({:?}) is invalid", v);
              return true; // consume this event anyway
            }
            new_time[i] = n.unwrap();
          }
          // use it as a new time
          self.set_time(new_time);

        // [10, 20, 30]
        } else if v.is_varray() {
          let mut new_time = Time::default();
          for (i, n) in v.values().take(3).map(|n| n.to_int()).enumerate() {
            if n.is_none() {
              eprintln!("clock::set_value({:?}) is invalid", v);
              return true;
            }
            new_time[i] = n.unwrap() as u8
          }
          // use it as a new time
          self.set_time(new_time);
        } else {
          // unknown format
          eprintln!("clock::set_value({:?}) is unsupported", v);
          return true;
        }
      }

      _ => {
        // unsupported event, skip it
        return false;
      }
    }

    // mark this event as handled (consume it)
    return true;
  }

  /// Redraw our element on each tick.
  fn on_timer(&mut self, root: HELEMENT, _timer_id: u64) -> bool {
    if self.update_time() {
      // redraw our clock
      Element::from(root).refresh().expect("Can't refresh element");
    }
    true
  }

  /// Request to draw our element.
  fn on_draw(&mut self, _root: HELEMENT, gfx: HGFX, area: &RECT, layer: DRAW_EVENTS) -> bool {
    if layer == DRAW_EVENTS::DRAW_CONTENT {
      // draw content only
      // leave the back- and foreground to be default
      let mut gfx = Graphics::from(gfx);
			self
				.draw_clock(&mut gfx, &area)
				.map_err(|e| println!("error in draw_clock: {:?}", e) )
				.ok();
		}

    // allow default drawing anyway
    return false;
  }
}

// 360°
const PI2: f32 = 2.0 * std::f32::consts::PI;

impl Clock {
  /// Update current time and say if changed.
  fn update_time(&mut self) -> bool {
    if self.is_frozen {
      return false;
    }

    // ask our script for the current time
    if let Some(now) = self.get_time() {
      let update = self.now != now;
      self.now = now;
      update
    } else {
      false
    }
  }

  /// Set the new time and redraw our element.
  fn set_time(&mut self, new_time: Time) {
    // set new time and redraw our clock
    self.now = new_time;
    if let Some(el) = self.element.as_ref() {
      el.refresh().ok();
    }
  }

  /// Get current time from script.
  fn get_time(&self) -> Option<Time> {
    let el = self.element.as_ref().unwrap();
    let script_func = if self.is_frozen { "getLocalTime" } else { "getUtcTime" };
    if let Ok(time) = el.call_function(script_func, &make_args!(self.gmt as i32)) {
      assert_eq!(time.len(), 3);
      let mut now = Time::default();
      for (i, n) in time.values().take(3).map(|n| n.to_int()).enumerate() {
        now[i] = n.unwrap() as u8;
      }
      Some(now)
    } else {
      eprintln!("error: can't eval get time script");
      None
    }
  }

  /// Draw our element.
  fn draw_clock(&mut self, gfx: &mut Graphics, area: &RECT) -> graphics::Result<()> {
    // save previous state
    let mut gfx = gfx.save_state()?;

    // setup our attributes
    let left = area.left as f32;
    let top = area.top as f32;
    let width = area.width() as f32;
    let height = area.height() as f32;

    let scale = if width < height { width / 300.0 } else { height / 300.0 };

    // translate to its center and rotate 45° left.
    gfx
      .translate((left + width / 2.0, top + height / 2.0))?
      .scale((scale, scale))?
      .rotate(-PI2 / 4.)?;

    gfx.line_color(0)?.line_cap(graphics::LINE_CAP::ROUND)?;

    // draw clock background
    self.draw_outline(&mut *gfx)?;

    // draw clock sticks
		self.draw_time(&mut *gfx)?;

    Ok(())
  }

  /// Draw clock static area (hour/minute marks).
  fn draw_outline(&mut self, gfx: &mut Graphics) -> graphics::Result<()> {
    // hour marks (every 5 ticks)
    {
      let mut gfx = gfx.save_state()?;
      gfx.line_width(8.0)?.line_color(rgb(0x32, 0x5F, 0xA2))?;

      for _ in 0..12 {
        gfx.rotate(PI2 / 12.)?.line((137., 0.), (144., 0.))?;
      }
    }

    // minute marks (every but 5th tick)
    {
      let mut gfx = gfx.save_state()?;
      gfx.line_width(3.0)?.line_color(rgb(0xA5, 0x2A, 0x2A))?;

      for i in 0..60 {
        if i % 5 != 0 {
          // skip hours
          gfx.line((143., 0.), (146., 0.))?;
        }
        gfx.rotate(PI2 / 60.)?;
      }
    }
    Ok(())
  }

  /// Draw clock arrows.
  fn draw_time(&mut self, gfx: &mut Graphics) -> graphics::Result<()> {
    let time = &self.now;
    let hours = f32::from(time[0]);
    let minutes = f32::from(time[1]);
    let seconds = f32::from(time[2]);

    {
      // hours
      let mut gfx = gfx.save_state()?;

      // 2PI*/12, 2PI/720,
      gfx.rotate(hours * (PI2 / 12 as f32) + minutes * (PI2 / (12 * 60) as f32) + seconds * (PI2 / (12 * 60 * 60) as f32))?;

      gfx
        .line_width(14.0)?
        .line_color(rgb(0x32, 0x5F, 0xA2))?
        .line((-20., 0.), (70., 0.))?;
    }
    {
      // minutes
      let mut gfx = gfx.save_state()?;

      gfx.rotate(minutes * (PI2 / 60 as f32) + seconds * (PI2 / (60 * 60) as f32))?;

      gfx
        .line_width(10.0)?
        .line_color(rgb(0x32, 0x5F, 0xA2))?
        .line((-28., 0.), (100., 0.))?;
    }
    {
      // seconds
      let mut gfx = gfx.save_state()?;

      gfx.rotate(seconds * (PI2 / 60 as f32))?;

      gfx
        .line_width(6.0)?
        .line_color(rgb(0xD4, 0, 0))?
        .fill_color(rgb(0xD4, 0, 0))?
        .line((-30., 0.), (83., 0.))?
        .circle((0., 0.), 10.)?;
    }
    Ok(())
  }
}


////////////////////////////////////
#[derive(Default)]
struct Text;

impl sciter::EventHandler for Text {
  fn get_subscription(&mut self) -> Option<EVENT_GROUPS> {
    Some(EVENT_GROUPS::HANDLE_DRAW)
  }

  fn attached(&mut self, _root: HELEMENT) {
	}

	fn on_draw(&mut self, _root: HELEMENT, gfx: HGFX, area: &RECT, layer: DRAW_EVENTS) -> bool {
    if layer == DRAW_EVENTS::DRAW_CONTENT {
      // draw content only
      // leave the back- and foreground to be default
			let mut gfx = Graphics::from(gfx);
			let e = Element::from(_root);
			self
				.draw_text(&e, &mut gfx, &area)
				.map_err(|e| println!("error in draw_clock: {:?}", e) )
				.ok();

				return true;
		}

    // allow default drawing anyway
    return false;
  }
}

impl Text {
  fn draw_text(&mut self, e: &Element, gfx: &mut Graphics, area: &RECT) -> graphics::Result<()> {

    // save previous state
    let mut gfx = gfx.save_state()?;

		// setup our attributes
    // let left = area.left as f32;
    // let top = area.top as f32;
    // let width = area.width() as f32;
		// let height = area.height() as f32;

		// println!("text::draw on {} at {} {} {} {}", e, left, top, width, height);

		use sciter::graphics::Text;

		let t = Text::with_style(&e, "native text", "font-style: italic")?;
		gfx.draw_text(&t, (area.left as f32, area.top as f32), 7)?;

		Ok(())
	}
}

fn main() {
  let mut frame = sciter::WindowBuilder::main_window().with_size((800, 600)).create();
  frame.register_behavior("native-clock", || Box::new(Clock::default()));
  frame.register_behavior("native-text", || Box::new(Text::default()));
  frame.load_html(include_bytes!("clock.htm"), Some("example://clock.htm"));
  frame.run_app();
}
