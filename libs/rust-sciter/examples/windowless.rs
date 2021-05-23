//! Windowless mode example (for Sciter.Lite build).
extern crate sciter;
extern crate winit;
extern crate winapi;
extern crate raw_window_handle;


fn main() {
	// "Windowless" Sciter builds are incompatible with the regular ones.
	if !cfg!(feature = "windowless") {
		panic!("This example requires \"windowless\" feature!");
	}

	// We need this to explicitly set path to the windowless sciter dll.
	if !cfg!(feature = "dynamic") {
		panic!("This example requires the \"dynamic\" feature enabled.")
	}

	if let Some(arg) = std::env::args().nth(1) {
		println!("loading sciter from {:?}", arg);
		if let Err(_) = sciter::set_options(sciter::RuntimeOptions::LibraryPath(&arg)) {
			panic!("Invalid sciter-lite dll specified.");
		}
	}

	// prepare and create a new window
	println!("create window");
	let mut events = winit::EventsLoop::new();

	use raw_window_handle::HasRawWindowHandle;
	let wnd = winit::WindowBuilder::new();
	let wnd = wnd.build(&events).expect("Failed to create window");
	let window_handle = wnd.raw_window_handle();

	// configure Sciter
	println!("create sciter instance");
	sciter::set_options(sciter::RuntimeOptions::UxTheming(true)).unwrap();
	sciter::set_options(sciter::RuntimeOptions::DebugMode(true)).unwrap();
	sciter::set_options(sciter::RuntimeOptions::ScriptFeatures(0xFF)).unwrap();

	// create an engine instance with an opaque pointer as an identifier
	use sciter::windowless::{Message, handle_message};
	let scwnd = { &wnd as *const _ as sciter::types::HWINDOW };
	handle_message(scwnd, Message::Create { backend: sciter::types::GFX_LAYER::SKIA_OPENGL, transparent: false, });

	#[cfg(windows)]
	{
		// Windows-specific: we need to redraw window in responce to the corresponding notification.
		// winit 0.20 has an explicit `Window::request_redraw` method,
		// here we use `winapi::InvalidateRect` for this.
		struct WindowlessHandler {
			hwnd: winapi::shared::windef::HWND,
		}

		impl sciter::HostHandler for WindowlessHandler {
			fn on_invalidate(&mut self, pnm: &sciter::host::SCN_INVALIDATE_RECT) {
				unsafe {
					let rc = &pnm.invalid_rect;
					let dst = winapi::shared::windef::RECT {
						left: rc.left,
						top: rc.top,
						right: rc.right,
						bottom: rc.bottom,
					};
					winapi::um::winuser::InvalidateRect(self.hwnd, &dst as *const _, 0);
					// println!("- {} {}", rc.width(), rc.height());
				}
			}
		}

		let handler = WindowlessHandler {
			hwnd: match window_handle {
				raw_window_handle::RawWindowHandle::Windows(data) => data.hwnd as winapi::shared::windef::HWND,
				_ => unreachable!(),
			},
		};

		let instance = sciter::Host::attach_with(scwnd, handler);

		let html = include_bytes!("minimal.htm");
		instance.load_html(html, Some("example://minimal.htm"));
	}

	// events processing
	use sciter::windowless::{MouseEvent, KeyboardEvent, RenderEvent};
	use sciter::windowless::{MOUSE_BUTTONS, MOUSE_EVENTS, KEYBOARD_STATES, KEY_EVENTS};

	let mut mouse_button = MOUSE_BUTTONS::NONE;
	let mut mouse_pos = (0, 0);

	let as_keys = |modifiers: winit::ModifiersState| {
		let mut keys = 0;
		if modifiers.ctrl {
			keys |= 0x01;
		}
		if modifiers.shift {
			keys |= 0x02;
		}
		if modifiers.alt {
			keys |= 0x04;
		}
		KEYBOARD_STATES::from(keys)
	};

	println!("running...");
	use winit::{Event, WindowEvent};
	let skip = ();
	let mut poll_break = false;
	let startup = std::time::Instant::now();
	loop {
	// release CPU a bit, hackish
	std::thread::sleep(std::time::Duration::from_millis(0));

	// Sciter processes timers and fading effects here
	handle_message(scwnd, Message::Heartbit {
		milliseconds: std::time::Instant::now().duration_since(startup).as_millis() as u32,
	});

	// the actual event loop polling
	events.poll_events(|event: winit::Event| {
		match event {
			Event::WindowEvent { event, window_id: _ } => {
				match event {
					WindowEvent::Destroyed => {
						// never called due to loop break on close
						println!("destroy");
						handle_message(scwnd, Message::Destroy);
						poll_break = true;
					},

					WindowEvent::CloseRequested => {
						println!("close");
						poll_break = true;
					},

					WindowEvent::Resized(size) => {
						// println!("{:?}, size: {:?}", event, size);
						let (width, height): (u32, u32) = size.into();
						handle_message(scwnd, Message::Size { width, height });
						skip
					},

					WindowEvent::Refresh => {

						let on_render = move |bitmap_area: &sciter::types::RECT, bitmap_data: &[u8]|
						{
							#[cfg(unix)]
							{
								let _ = bitmap_area;
								let _ = bitmap_data;
								let _ = window_handle;
							}

							// Windows-specific bitmap rendering on the window
							#[cfg(windows)]
							{
								use winapi::um::winuser::*;
								use winapi::um::wingdi::*;
								use winapi::shared::minwindef::LPVOID;

								let hwnd = match window_handle {
									raw_window_handle::RawWindowHandle::Windows(data) => data.hwnd as winapi::shared::windef::HWND,
									_ => unreachable!(),
								};

								unsafe {
									// NOTE: we use `GetDC` here instead of `BeginPaint`, because the way
									// winit 0.19 processed the `WM_PAINT` message (it always calls `DefWindowProcW`).

									// let mut ps = PAINTSTRUCT::default();
									// let hdc = BeginPaint(hwnd, &mut ps as *mut _);

									let hdc = GetDC(hwnd);

									let (w, h) = (bitmap_area.width(), bitmap_area.height());

									let mem_dc = CreateCompatibleDC(hdc);
									let mem_bm = CreateCompatibleBitmap(hdc, w, h);

									let mut bmi = BITMAPINFO::default();
									{
										let mut info = &mut bmi.bmiHeader;
										info.biSize = std::mem::size_of::<BITMAPINFO>() as u32;
										info.biWidth = w;
										info.biHeight = -h;
										info.biPlanes = 1;
										info.biBitCount = 32;
									}

									let old_bm = SelectObject(mem_dc, mem_bm as LPVOID);

									let _copied = StretchDIBits(mem_dc, 0, 0, w, h, 0, 0, w, h, bitmap_data.as_ptr() as *const _, &bmi as *const _, 0, SRCCOPY);
									let _ok = BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

									SelectObject(mem_dc, old_bm);

									// EndPaint(hwnd, &ps as *const _);
									ReleaseDC(hwnd, hdc);

									// println!("+ {} {}", w, h);
								}
							}

						};

						let cb = RenderEvent {
							layer: None,
							callback: Box::new(on_render),
						};

						handle_message(scwnd, Message::RenderTo(cb));
						skip
					},

					WindowEvent::Focused(enter) => {
						println!("focus {}", enter);
						handle_message(scwnd, Message::Focus { enter });
						skip
					},

					WindowEvent::CursorEntered { device_id: _ } => {
						println!("mouse enter");
						let event = MouseEvent {
							event: MOUSE_EVENTS::MOUSE_ENTER,
							button: mouse_button,
							modifiers: KEYBOARD_STATES::from(0),
							pos: sciter::types::POINT {
								x: mouse_pos.0,
								y: mouse_pos.1,
							},
						};

						handle_message(scwnd, Message::Mouse(event));
						skip
					},

					WindowEvent::CursorLeft { device_id: _ } => {
						println!("mouse leave");
						let event = MouseEvent {
							event: MOUSE_EVENTS::MOUSE_LEAVE,
							button: mouse_button,
							modifiers: KEYBOARD_STATES::from(0),
							pos: sciter::types::POINT {
								x: mouse_pos.0,
								y: mouse_pos.1,
							},
						};

						handle_message(scwnd, Message::Mouse(event));
						skip
					},

					WindowEvent::CursorMoved { device_id: _, position, modifiers } => {
						mouse_pos = position.into();

						let event = MouseEvent {
							event: MOUSE_EVENTS::MOUSE_MOVE,
							button: mouse_button,
							modifiers: as_keys(modifiers),
							pos: sciter::types::POINT {
								x: mouse_pos.0,
								y: mouse_pos.1,
							},
						};

						handle_message(scwnd, Message::Mouse(event));
						skip
					},

					WindowEvent::MouseInput { device_id: _, state, button, modifiers } => {
						mouse_button = match button {
							winit::MouseButton::Left => MOUSE_BUTTONS::MAIN,
							winit::MouseButton::Right => MOUSE_BUTTONS::PROP,
							winit::MouseButton::Middle => MOUSE_BUTTONS::MIDDLE,
							_ => MOUSE_BUTTONS::NONE,
						};
						println!("mouse {:?} as {:?}", mouse_button, mouse_pos);

						let event = MouseEvent {
							event: if state == winit::ElementState::Pressed { MOUSE_EVENTS::MOUSE_DOWN } else { MOUSE_EVENTS::MOUSE_UP },
							button: mouse_button,
							modifiers: as_keys(modifiers),
							pos: sciter::types::POINT {
								x: mouse_pos.0,
								y: mouse_pos.1,
							},
						};

						handle_message(scwnd, Message::Mouse(event));
						skip
					},

					WindowEvent::KeyboardInput { device_id: _, input } => {
						println!("key {} {}", input.scancode, if input.state == winit::ElementState::Pressed { "down" } else { "up" });

						let event = KeyboardEvent {
							event: if input.state == winit::ElementState::Pressed { KEY_EVENTS::KEY_DOWN } else { KEY_EVENTS::KEY_UP },
							code: input.scancode,
							modifiers: as_keys(input.modifiers),
						};

						handle_message(scwnd, Message::Keyboard(event));
						skip
					},

					_	=> (),
				}
			},

			_ => (),
		}
	});

	if poll_break {
		break;
	}
	}

	println!("done, quit");
}
