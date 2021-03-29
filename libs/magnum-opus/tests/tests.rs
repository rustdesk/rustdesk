extern crate magnum_opus;

fn check_ascii(s: &str) -> &str {
	for &b in s.as_bytes() {
		assert!(b < 0x80, "Non-ASCII character in string");
		assert!(b > 0x00, "NUL in string")
	}
	std::str::from_utf8(s.as_bytes()).unwrap()
}

#[test]
fn strings_ascii() {
	use magnum_opus::ErrorCode::*;

	println!("\nVersion: {}", check_ascii(magnum_opus::version()));

	let codes = [BadArg, BufferTooSmall, InternalError, InvalidPacket,
		Unimplemented, InvalidState, AllocFail, Unknown];
	for &code in codes.iter() {
		println!("{:?}: {}", code, check_ascii(code.description()));
	}
}

// 48000Hz * 1 channel * 20 ms / 1000
const MONO_20MS: usize = 48000 * 1 * 20 / 1000;

#[test]
fn encode_mono() {
	let mut encoder = magnum_opus::Encoder::new(48000, magnum_opus::Channels::Mono, magnum_opus::Application::Audio).unwrap();

	let mut output = [0; 256];
	let len = encoder.encode(&[0_i16; MONO_20MS], &mut output).unwrap();
	assert_eq!(&output[..len], &[248, 255, 254]);

	let len = encoder.encode(&[0_i16; MONO_20MS], &mut output).unwrap();
	assert_eq!(&output[..len], &[248, 255, 254]);

	let len = encoder.encode(&[1_i16; MONO_20MS], &mut output).unwrap();
	assert!(len > 190 && len < 220);

	let len = encoder.encode(&[0_i16; MONO_20MS], &mut output).unwrap();
	assert!(len > 170 && len < 190);

	let myvec = encoder.encode_vec(&[1_i16; MONO_20MS], output.len()).unwrap();
	assert!(myvec.len() > 120 && myvec.len() < 140);
}

#[test]
fn encode_stereo() {
	let mut encoder = magnum_opus::Encoder::new(48000, magnum_opus::Channels::Stereo, magnum_opus::Application::Audio).unwrap();

	let mut output = [0; 512];
	let len = encoder.encode(&[0_i16; 2 * MONO_20MS], &mut output).unwrap();
	assert_eq!(&output[..len], &[252, 255, 254]);

	let len = encoder.encode(&[0_i16; 4 * MONO_20MS], &mut output).unwrap();
	assert_eq!(&output[..len], &[253, 255, 254, 255, 254]);

	let len = encoder.encode(&[17_i16; 2 * MONO_20MS], &mut output).unwrap();
	assert!(len > 240);

	let len = encoder.encode(&[0_i16; 2 * MONO_20MS], &mut output).unwrap();
	assert!(len > 240);

	// Very small buffer should still succeed
	let len = encoder.encode(&[95_i16; 2 * MONO_20MS], &mut [0; 20]).unwrap();
	assert!(len <= 20);

	let myvec = encoder.encode_vec(&[95_i16; 2 * MONO_20MS], 20).unwrap();
	assert!(myvec.len() <= 20);
}

#[test]
fn encode_bad_rate() {
	match magnum_opus::Encoder::new(48001, magnum_opus::Channels::Mono, magnum_opus::Application::Audio) {
		Ok(_) => panic!("Encoder::new did not return BadArg"),
		Err(err) => assert_eq!(err.code(), magnum_opus::ErrorCode::BadArg),
	}
}

#[test]
fn encode_bad_buffer() {
	let mut encoder = magnum_opus::Encoder::new(48000, magnum_opus::Channels::Stereo, magnum_opus::Application::Audio).unwrap();
	match encoder.encode(&[1_i16; 2 * MONO_20MS], &mut [0; 0]) {
		Ok(_) => panic!("encode with 0-length buffer did not return BadArg"),
		Err(err) => assert_eq!(err.code(), magnum_opus::ErrorCode::BadArg),
	}
}

#[test]
fn repacketizer() {
	let mut rp = magnum_opus::Repacketizer::new().unwrap();
	let mut out = [0; 256];

	for _ in 0..2 {
		let packet1 = [249, 255, 254, 255, 254];
		let packet2 = [248, 255, 254];

		let mut state = rp.begin();
		state.cat(&packet1).unwrap();
		state.cat(&packet2).unwrap();
		let len = state.out(&mut out).unwrap();
		assert_eq!(&out[..len], &[251, 3, 255, 254, 255, 254, 255, 254]);
	}
	for _ in 0..2 {
		let packet = [248, 255, 254];
		let state = rp.begin().cat_move(&packet).unwrap();
		let packet = [249, 255, 254, 255, 254];
		let state = state.cat_move(&packet).unwrap();
		let len = {state}.out(&mut out).unwrap();
		assert_eq!(&out[..len], &[251, 3, 255, 254, 255, 254, 255, 254]);
	}
	for _ in 0..2 {
		let len = rp.combine(&[
			&[249, 255, 254, 255, 254],
			&[248, 255, 254],
		], &mut out).unwrap();
		assert_eq!(&out[..len], &[251, 3, 255, 254, 255, 254, 255, 254]);
	}
	for _ in 0..2 {
		let len = rp.begin()
			.cat_move(&[248, 255, 254]).unwrap()
			.cat_move(&[248, 71, 71]).unwrap()
			.out(&mut out).unwrap();
		assert_eq!(&out[..len], &[249, 255, 254, 71, 71]);
	}
}
