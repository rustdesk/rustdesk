//! Sciter sample with archived resources.

extern crate sciter;

fn main() {
  let resources = include_bytes!("archived.rc");

  let mut frame = sciter::WindowBuilder::main_window()
  	.fixed()
  	.with_size((600, 400))
  	.create();

  frame.archive_handler(resources).expect("Invalid archive");

  frame.load_file("this://app/index.htm");
  frame.run_app();
}
