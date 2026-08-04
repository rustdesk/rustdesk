#[cfg(not(target_pointer_width = "64"))]
fn main() {
    eprintln!(
        "This example requires a 64-bit target because SVT-AV1 does not support 32-bit targets."
    );
    std::process::exit(1);
}

// The RustDesk application normally provides this symbol from src/platform/macos.mm.
// Standalone scrap examples need their own CoreGraphics-based implementation.
#[cfg(all(target_pointer_width = "64", target_os = "macos"))]
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn BackingScaleFactor(display: u32) -> f32 {
    let pixels = unsafe { scrap::quartz::ffi::CGDisplayPixelsWide(display) } as f64;
    let bounds = unsafe { scrap::quartz::ffi::CGDisplayBounds(display) };
    if pixels > 0.0 && bounds.size.width > 0.0 {
        (pixels / bounds.size.width).max(1.0) as f32
    } else {
        1.0
    }
}

#[cfg(target_pointer_width = "64")]
fn main() {
    if let Err(error) = benchmark::run() {
        eprintln!("AV1 benchmark failed: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(target_pointer_width = "64")]
mod benchmark {
    use hbb_common::{
        anyhow::{anyhow, bail},
        bytes::Bytes,
        message_proto::{video_frame, Chroma, EncodedVideoFrame, EncodedVideoFrames, VideoFrame},
        ResultType,
    };
    use scrap::{
        aom::{AomDecoder, AomEncoder, AomEncoderConfig},
        codec::{EncoderApi, EncoderCfg},
        record::{RecordState, Recorder, RecorderContext},
        svt_av1::{SvtAv1Encoder, SvtAv1EncoderConfig},
        Capturer, Display as ScrapDisplay, EncodeInput, EncodeYuvFormat, GoogleImage, Pixfmt,
        TraitCapturer, STRIDE_ALIGN,
    };
    use std::{
        convert::TryFrom,
        env,
        fmt::Display,
        io::ErrorKind,
        str::FromStr,
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    const USAGE: &str = "\
Compare RustDesk's AOM and SVT-AV1 encoder configurations with identical I420 frames.

Usage:
  cargo run -p scrap --release --example av1_benchmark -- [options]

Options:
  --input=TYPE    Input type: synthetic or screenshot [default: synthetic]
  --capture-delay=N
                  Seconds to wait before capturing the screenshot sequence [default: 0]
  --width=N       Synthetic frame width [default: 1920]
  --height=N      Synthetic frame height [default: 1080]
  --frames=N      Measured frames per encoder [default: 300]
  --warmup=N      Unmeasured warm-up frames per encoder [default: 10]
  --fps=N         Configured frame rate [default: 30]
  --quality=N     RustDesk quality ratio, in (0, 2] [default: 1.0]
  --svt-preset=N  SVT-AV1 RTC preset, from 7 (slow) to 13 (fast) [default: 8]
  --svt-first     Run SVT-AV1 before AOM to help check order/thermal bias
  --record        Save both encoded streams under target/av1-benchmark-recordings
  -h, --help      Show this help
";

    const SCREENSHOT_INTERVAL: Duration = Duration::from_millis(30);

    #[derive(Clone, Copy)]
    struct Args {
        input: InputKind,
        capture_delay: u64,
        width: u32,
        height: u32,
        frames: usize,
        warmup: usize,
        fps: u32,
        quality: f32,
        svt_preset: i8,
        svt_first: bool,
        record: bool,
    }

    impl Default for Args {
        fn default() -> Self {
            Self {
                input: InputKind::Synthetic,
                capture_delay: 0,
                width: 1920,
                height: 1080,
                frames: 300,
                warmup: 10,
                fps: 30,
                quality: 1.0,
                svt_preset: 8,
                svt_first: false,
                record: false,
            }
        }
    }

    #[derive(Clone, Copy)]
    enum InputKind {
        Synthetic,
        Screenshot,
    }

    impl InputKind {
        fn name(self) -> &'static str {
            match self {
                Self::Synthetic => "synthetic",
                Self::Screenshot => "screenshot",
            }
        }
    }

    impl FromStr for InputKind {
        type Err = &'static str;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "synthetic" => Ok(Self::Synthetic),
                "screenshot" => Ok(Self::Screenshot),
                _ => Err("expected synthetic or screenshot"),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum EncoderKind {
        Aom,
        SvtAv1,
    }

    impl EncoderKind {
        fn name(self) -> &'static str {
            match self {
                Self::Aom => "aom",
                Self::SvtAv1 => "svt-av1",
            }
        }
    }

    struct BenchmarkResult {
        encoder: &'static str,
        initialization: Duration,
        first_frame: Duration,
        samples: Vec<Duration>,
        output_frames: usize,
        output_bytes: u64,
        pts_mismatches: usize,
        decoded_frames: usize,
        quality: QualityMetrics,
        fps: u32,
        reference_format: EncodeYuvFormat,
        packets: Vec<EncodedPacket>,
    }

    struct EncodedPacket {
        input_index: usize,
        pts: i64,
        key: bool,
        data: Vec<u8>,
    }

    impl BenchmarkResult {
        fn total_encode_time(&self) -> Duration {
            self.samples.iter().copied().sum()
        }

        fn initialization_ms(&self) -> f64 {
            duration_ms(self.initialization)
        }

        fn first_frame_ms(&self) -> f64 {
            duration_ms(self.first_frame)
        }

        fn mean_ms(&self) -> f64 {
            duration_ms(self.total_encode_time()) / self.samples.len() as f64
        }

        fn percentile_ms(&self, percentile: f64) -> f64 {
            let mut samples = self.samples.clone();
            samples.sort_unstable();
            let index = ((samples.len() - 1) as f64 * percentile).round() as usize;
            duration_ms(samples[index])
        }

        fn encode_fps(&self) -> f64 {
            self.samples.len() as f64 / self.total_encode_time().as_secs_f64()
        }

        fn average_bytes(&self) -> f64 {
            self.output_bytes as f64 / self.output_frames as f64
        }

        fn bitrate_kbps(&self) -> f64 {
            self.output_bytes as f64 * 8.0 * self.fps as f64 / self.samples.len() as f64 / 1000.0
        }
    }

    #[derive(Default)]
    struct PlaneError {
        squared_error: u128,
        samples: u128,
    }

    impl PlaneError {
        fn psnr(&self) -> f64 {
            if self.squared_error == 0 {
                return f64::INFINITY;
            }
            let mse = self.squared_error as f64 / self.samples as f64;
            10.0 * (255.0f64 * 255.0 / mse).log10()
        }
    }

    #[derive(Default)]
    struct QualityMetrics {
        y: PlaneError,
        u: PlaneError,
        v: PlaneError,
        ssim_y_sum: f64,
        ssim_y_windows: u64,
        frames: usize,
    }

    impl QualityMetrics {
        fn add_frame(
            &mut self,
            reference: &[u8],
            reference_format: &EncodeYuvFormat,
            decoded: &scrap::aom::Image,
        ) -> ResultType<()> {
            if decoded.width() != reference_format.w || decoded.height() != reference_format.h {
                bail!(
                    "decoded dimensions {}x{} do not match reference {}x{}",
                    decoded.width(),
                    decoded.height(),
                    reference_format.w,
                    reference_format.h
                );
            }
            if decoded.chroma() != Chroma::I420 {
                bail!("decoded image is not I420: {:?}", decoded.chroma());
            }
            let strides = decoded.stride();
            let planes = decoded.planes();
            if strides.len() < 3 || planes.len() < 3 {
                bail!("decoded image has fewer than three planes");
            }
            if strides[..3].iter().any(|stride| *stride <= 0)
                || planes[..3].iter().any(|plane| plane.is_null())
            {
                bail!("decoded image has an invalid plane or stride");
            }

            let chroma_width = (reference_format.w + 1) / 2;
            let chroma_height = (reference_format.h + 1) / 2;
            accumulate_plane_error(
                &mut self.y,
                reference,
                0,
                reference_format.stride[0],
                planes[0],
                strides[0] as usize,
                reference_format.w,
                reference_format.h,
            );
            accumulate_plane_error(
                &mut self.u,
                reference,
                reference_format.u,
                reference_format.stride[1],
                planes[1],
                strides[1] as usize,
                chroma_width,
                chroma_height,
            );
            accumulate_plane_error(
                &mut self.v,
                reference,
                reference_format.v,
                reference_format.stride[2],
                planes[2],
                strides[2] as usize,
                chroma_width,
                chroma_height,
            );
            let (ssim_sum, windows) = calculate_ssim_y(
                reference,
                reference_format.stride[0],
                planes[0],
                strides[0] as usize,
                reference_format.w,
                reference_format.h,
            );
            self.ssim_y_sum += ssim_sum;
            self.ssim_y_windows += windows;
            self.frames += 1;
            Ok(())
        }

        fn psnr_yuv(&self) -> f64 {
            let squared_error = self.y.squared_error + self.u.squared_error + self.v.squared_error;
            if squared_error == 0 {
                return f64::INFINITY;
            }
            let samples = self.y.samples + self.u.samples + self.v.samples;
            let mse = squared_error as f64 / samples as f64;
            10.0 * (255.0f64 * 255.0 / mse).log10()
        }

        fn ssim_y(&self) -> f64 {
            self.ssim_y_sum / self.ssim_y_windows as f64
        }
    }

    struct SyntheticI420 {
        format: EncodeYuvFormat,
        base: Vec<u8>,
        frame: Vec<u8>,
    }

    struct ScreenshotSequenceI420 {
        format: EncodeYuvFormat,
        frames: Vec<Vec<u8>>,
    }

    enum BenchmarkInput {
        Synthetic,
        Screenshot(ScreenshotSequenceI420),
    }

    impl BenchmarkInput {
        fn frame_source(&self, format: EncodeYuvFormat) -> ResultType<FrameSource<'_>> {
            match self {
                Self::Synthetic => Ok(FrameSource::Synthetic(SyntheticI420::new(format))),
                Self::Screenshot(screenshot) => {
                    validate_matching_i420_layout(&screenshot.format, &format)?;
                    Ok(FrameSource::Screenshot(&screenshot.frames))
                }
            }
        }

        fn description(&self) -> &'static str {
            match self {
                Self::Synthetic => {
                    "Synthetic input: static checkerboard with a moving YUV rectangle (I420)"
                }
                Self::Screenshot(_) => {
                    "Screenshot input: primary-display frames captured at 30 ms intervals (I420)"
                }
            }
        }
    }

    enum FrameSource<'a> {
        Synthetic(SyntheticI420),
        Screenshot(&'a [Vec<u8>]),
    }

    impl FrameSource<'_> {
        fn frame(&mut self, index: usize) -> ResultType<&[u8]> {
            match self {
                Self::Synthetic(input) => Ok(input.frame(index)),
                Self::Screenshot(frames) => frames
                    .get(index)
                    .map(Vec::as_slice)
                    .ok_or_else(|| anyhow!("screenshot frame {index} is unavailable")),
            }
        }
    }

    impl SyntheticI420 {
        fn new(format: EncodeYuvFormat) -> Self {
            let chroma_height = (format.h + 1) / 2;
            let len = format.v + format.stride[2] * chroma_height;
            let mut base = vec![128u8; len];
            for y in 0..format.h {
                let row = &mut base[y * format.stride[0]..y * format.stride[0] + format.w];
                for (x, value) in row.iter_mut().enumerate() {
                    *value = if (x / 32 + y / 32) % 2 == 0 { 48 } else { 192 };
                }
            }
            let chroma_width = (format.w + 1) / 2;
            for y in 0..chroma_height {
                for x in 0..chroma_width {
                    let first = (x / 16 + y / 16) % 2 == 0;
                    base[format.u + y * format.stride[1] + x] = if first { 96 } else { 160 };
                    base[format.v + y * format.stride[2] + x] = if first { 160 } else { 96 };
                }
            }
            let frame = base.clone();
            Self {
                format,
                base,
                frame,
            }
        }

        fn frame(&mut self, index: usize) -> &[u8] {
            self.frame.copy_from_slice(&self.base);
            let rect_width = (self.format.w / 6).max(1);
            let rect_height = (self.format.h / 6).max(1);
            let x_range = self.format.w - rect_width + 1;
            let y_range = self.format.h - rect_height + 1;
            let x = index.saturating_mul(13) % x_range;
            let y = index.saturating_mul(7) % y_range;
            let value = 32 + (index.saturating_mul(17) % 192) as u8;
            for row in y..y + rect_height {
                let begin = row * self.format.stride[0] + x;
                self.frame[begin..begin + rect_width].fill(value);
            }
            let chroma_left = x / 2;
            let chroma_top = y / 2;
            let chroma_right = (x + rect_width + 1) / 2;
            let chroma_bottom = (y + rect_height + 1) / 2;
            let u_value = 64 + (index.saturating_mul(11) % 128) as u8;
            let v_value = 192 - (index.saturating_mul(7) % 128) as u8;
            for row in chroma_top..chroma_bottom {
                let u_begin = self.format.u + row * self.format.stride[1] + chroma_left;
                let v_begin = self.format.v + row * self.format.stride[2] + chroma_left;
                self.frame[u_begin..u_begin + chroma_right - chroma_left].fill(u_value);
                self.frame[v_begin..v_begin + chroma_right - chroma_left].fill(v_value);
            }
            &self.frame
        }
    }

    fn validate_matching_i420_layout(
        captured: &EncodeYuvFormat,
        encoder: &EncodeYuvFormat,
    ) -> ResultType<()> {
        let matching_strides = captured.stride.get(..3) == encoder.stride.get(..3);
        if captured.pixfmt != Pixfmt::I420
            || encoder.pixfmt != Pixfmt::I420
            || captured.w != encoder.w
            || captured.h != encoder.h
            || !matching_strides
            || captured.u != encoder.u
            || captured.v != encoder.v
        {
            bail!(
                "captured I420 layout does not match encoder layout: captured={captured:?}, encoder={encoder:?}"
            );
        }
        Ok(())
    }

    fn aligned_i420_format(width: usize, height: usize) -> ResultType<EncodeYuvFormat> {
        let align = |value: usize| {
            value
                .checked_add(STRIDE_ALIGN - 1)
                .map(|value| value & !(STRIDE_ALIGN - 1))
                .ok_or_else(|| anyhow!("screen dimensions are too large"))
        };
        let stride_y = align(width)?;
        let stride_uv = align((width + 1) / 2)?;
        let u = stride_y
            .checked_mul(height)
            .ok_or_else(|| anyhow!("screen dimensions are too large"))?;
        let chroma_height = (height + 1) / 2;
        let v = stride_uv
            .checked_mul(chroma_height)
            .and_then(|chroma_len| u.checked_add(chroma_len))
            .ok_or_else(|| anyhow!("screen dimensions are too large"))?;
        Ok(EncodeYuvFormat {
            pixfmt: Pixfmt::I420,
            w: width,
            h: height,
            stride: vec![stride_y, stride_uv, stride_uv],
            u,
            v,
        })
    }

    fn capture_primary_screenshots(
        delay_seconds: u64,
        frame_count: usize,
    ) -> ResultType<ScreenshotSequenceI420> {
        let mut displays = ScrapDisplay::all()?;
        if displays.is_empty() {
            bail!("no displays are available for screenshot input");
        }
        let primary_index = match displays.iter().position(ScrapDisplay::is_primary) {
            Some(index) => index,
            None => 0,
        };
        let display = displays.remove(primary_index);
        let mut capturer = Capturer::new(display)?;
        let width = capturer.width();
        let height = capturer.height();
        if width == 0 || height == 0 {
            bail!("primary display reported an invalid size: {width}x{height}");
        }
        let format = aligned_i420_format(width, height)?;

        for remaining in (1..=delay_seconds).rev() {
            println!("Taking primary-display screenshot in {remaining}s...");
            thread::sleep(Duration::from_secs(1));
        }
        println!(
            "Capturing {frame_count} primary-display frames at {width}x{height} with a {} ms interval...",
            SCREENSHOT_INTERVAL.as_millis()
        );

        let wait_limit = Duration::from_secs(10);
        let mut wait_start = Instant::now();
        let mut frames = Vec::with_capacity(frame_count);
        let mut yuv = Vec::new();
        let mut mid_data = Vec::new();
        while frames.len() < frame_count {
            match capturer.frame(Duration::from_millis(100)) {
                Ok(frame) if !frame.valid() => {}
                Ok(frame) => {
                    let converted = frame.to(format.clone(), &mut yuv, &mut mid_data)?;
                    if converted.yuv().is_err() {
                        bail!("screen capturer returned a GPU texture instead of CPU pixels");
                    }
                    frames.push(std::mem::take(&mut yuv));
                    wait_start = Instant::now();
                    if frames.len() < frame_count {
                        thread::sleep(SCREENSHOT_INTERVAL);
                    }
                    continue;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => bail!("failed to capture primary display: {error}"),
            }
            if wait_start.elapsed() >= wait_limit {
                bail!(
                    "timed out waiting for screenshot frame {}/{}; check screen-recording permission and try again",
                    frames.len() + 1,
                    frame_count
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
        let captured_bytes = frames.iter().map(Vec::len).sum::<usize>();
        println!(
            "Screenshot sequence captured ({:.1} MiB).",
            captured_bytes as f64 / (1024.0 * 1024.0)
        );
        Ok(ScreenshotSequenceI420 { format, frames })
    }

    fn accumulate_plane_error(
        error: &mut PlaneError,
        reference: &[u8],
        reference_offset: usize,
        reference_stride: usize,
        decoded: *mut u8,
        decoded_stride: usize,
        width: usize,
        height: usize,
    ) {
        for row in 0..height {
            let reference_begin = reference_offset + row * reference_stride;
            let reference_row = &reference[reference_begin..reference_begin + width];
            // The decoder-owned plane and positive stride were validated by add_frame,
            // and width/height are the decoded image's visible dimensions.
            let decoded_row =
                unsafe { std::slice::from_raw_parts(decoded.add(row * decoded_stride), width) };
            for (&original, &reconstructed) in reference_row.iter().zip(decoded_row) {
                let difference = original as i32 - reconstructed as i32;
                error.squared_error += (difference * difference) as u128;
            }
        }
        error.samples += (width * height) as u128;
    }

    // Mean SSIM over non-overlapping 8x8 luma windows. Edge windows use their
    // visible size, so every displayed pixel contributes to the result.
    fn calculate_ssim_y(
        reference: &[u8],
        reference_stride: usize,
        decoded: *mut u8,
        decoded_stride: usize,
        width: usize,
        height: usize,
    ) -> (f64, u64) {
        const WINDOW: usize = 8;
        const C1: f64 = 6.5025; // (0.01 * 255)^2
        const C2: f64 = 58.5225; // (0.03 * 255)^2

        let mut total = 0.0;
        let mut windows = 0u64;
        for top in (0..height).step_by(WINDOW) {
            for left in (0..width).step_by(WINDOW) {
                let block_width = WINDOW.min(width - left);
                let block_height = WINDOW.min(height - top);
                let samples = (block_width * block_height) as f64;
                let mut original_sum = 0.0;
                let mut decoded_sum = 0.0;
                let mut original_square_sum = 0.0;
                let mut decoded_square_sum = 0.0;
                let mut product_sum = 0.0;

                for row in top..top + block_height {
                    let reference_begin = row * reference_stride + left;
                    let reference_row = &reference[reference_begin..reference_begin + block_width];
                    // The pointer range is covered by the validated decoded plane.
                    let decoded_row = unsafe {
                        std::slice::from_raw_parts(
                            decoded.add(row * decoded_stride + left),
                            block_width,
                        )
                    };
                    for (&original, &reconstructed) in reference_row.iter().zip(decoded_row) {
                        let original = original as f64;
                        let reconstructed = reconstructed as f64;
                        original_sum += original;
                        decoded_sum += reconstructed;
                        original_square_sum += original * original;
                        decoded_square_sum += reconstructed * reconstructed;
                        product_sum += original * reconstructed;
                    }
                }

                let original_mean = original_sum / samples;
                let decoded_mean = decoded_sum / samples;
                let original_variance =
                    (original_square_sum / samples - original_mean * original_mean).max(0.0);
                let decoded_variance =
                    (decoded_square_sum / samples - decoded_mean * decoded_mean).max(0.0);
                let covariance = product_sum / samples - original_mean * decoded_mean;
                total += ((2.0 * original_mean * decoded_mean + C1) * (2.0 * covariance + C2))
                    / ((original_mean * original_mean + decoded_mean * decoded_mean + C1)
                        * (original_variance + decoded_variance + C2));
                windows += 1;
            }
        }
        (total, windows)
    }

    pub fn run() -> ResultType<()> {
        let Some(mut args) = parse_args()? else {
            print!("{USAGE}");
            return Ok(());
        };
        let total_frames = args
            .warmup
            .checked_add(args.frames)
            .ok_or_else(|| anyhow!("--warmup + --frames is too large"))?;
        if args.frames == 0 {
            bail!("--frames must be greater than zero");
        }
        let input = match args.input {
            InputKind::Synthetic => BenchmarkInput::Synthetic,
            InputKind::Screenshot => {
                let screenshot = capture_primary_screenshots(args.capture_delay, total_frames)?;
                args.width = u32::try_from(screenshot.format.w)
                    .map_err(|_| anyhow!("captured screen width is too large"))?;
                args.height = u32::try_from(screenshot.format.h)
                    .map_err(|_| anyhow!("captured screen height is too large"))?;
                BenchmarkInput::Screenshot(screenshot)
            }
        };
        validate_args(args)?;

        println!(
            "AV1 benchmark: {}x{}, input={}, quality={}, fps={}, warmup={}, measured={}",
            args.width,
            args.height,
            args.input.name(),
            args.quality,
            args.fps,
            args.warmup,
            args.frames
        );
        println!("{}", input.description());
        println!("Timing scope: encode_to_message only; input generation is excluded");
        println!("Quality scope: separate decode passes after both encoders finish");
        println!("Quality metrics: sequence PSNR and mean non-overlapping 8x8 luma SSIM\n");
        let (svt_min_qp, svt_max_qp) = SvtAv1Encoder::quality_qp_range(args.quality);
        println!(
            "Comparison constraint: SVT-AV1 QP range={}..{} (matched to AOM for quality={}); preset=M{}\n",
            svt_min_qp, svt_max_qp, args.quality, args.svt_preset
        );

        let (mut aom, mut svt) = if args.svt_first {
            let svt = benchmark_encoder(EncoderKind::SvtAv1, args, &input)?;
            let aom = benchmark_encoder(EncoderKind::Aom, args, &input)?;
            (aom, svt)
        } else {
            let aom = benchmark_encoder(EncoderKind::Aom, args, &input)?;
            let svt = benchmark_encoder(EncoderKind::SvtAv1, args, &input)?;
            (aom, svt)
        };

        evaluate_quality(&mut aom, args, &input)?;
        evaluate_quality(&mut svt, args, &input)?;
        if args.record {
            write_recordings([&aom, &svt], args)?;
        }
        print_results(&aom, &svt);
        Ok(())
    }

    fn validate_args(args: Args) -> ResultType<()> {
        if args.frames == 0 {
            bail!("--frames must be greater than zero");
        }
        if args.fps == 0 || args.fps > 1000 {
            bail!("--fps must be in the range 1..=1000");
        }
        if !args.quality.is_finite() || args.quality <= 0.0 || args.quality > 2.0 {
            bail!("--quality must be in the range (0, 2]");
        }
        if !(7..=13).contains(&args.svt_preset) {
            bail!("--svt-preset must be in the range 7..=13");
        }
        if !SvtAv1Encoder::support(args.width, args.height) {
            bail!(
                "SVT-AV1 does not support resolution {}x{}; use even dimensions between 64x64 and 16384x8704",
                args.width,
                args.height
            );
        }
        Ok(())
    }

    fn benchmark_encoder(
        kind: EncoderKind,
        args: Args,
        benchmark_input: &BenchmarkInput,
    ) -> ResultType<BenchmarkResult> {
        println!("Running {}...", kind.name());
        let initialization_start = Instant::now();
        let mut encoder: Box<dyn EncoderApi> = match kind {
            EncoderKind::Aom => Box::new(AomEncoder::new(
                EncoderCfg::AOM(AomEncoderConfig {
                    width: args.width,
                    height: args.height,
                    quality: args.quality,
                    keyframe_interval: None,
                }),
                false,
            )?),
            EncoderKind::SvtAv1 => Box::new(SvtAv1Encoder::new(
                EncoderCfg::SVTAV1(SvtAv1EncoderConfig {
                    width: args.width,
                    height: args.height,
                    quality: args.quality,
                    keyframe_interval: None,
                    qp_range: Some(SvtAv1Encoder::quality_qp_range(args.quality)),
                    preset: Some(args.svt_preset),
                }),
                false,
            )?),
        };
        encoder.set_fps(args.fps);
        let initialization = initialization_start.elapsed();
        let reference_format = encoder.yuvfmt();
        let mut input = benchmark_input.frame_source(reference_format.clone())?;
        let total_frames = args
            .warmup
            .checked_add(args.frames)
            .ok_or_else(|| anyhow!("--warmup + --frames is too large"))?;
        let mut first_frame = Duration::ZERO;
        let mut samples = Vec::with_capacity(args.frames);
        let mut output_frames = 0usize;
        let mut output_bytes = 0u64;
        let mut pts_mismatches = 0usize;
        let mut packets = Vec::with_capacity(total_frames);

        for index in 0..total_frames {
            let pts = index.saturating_mul(1000) / args.fps as usize;
            let yuv = input.frame(index)?;
            let encode_start = Instant::now();
            let message = encoder.encode_to_message(EncodeInput::YUV(yuv), pts as i64)?;
            let elapsed = encode_start.elapsed();
            if index == 0 {
                first_frame = elapsed;
            }
            let measured = index >= args.warmup;
            let frames = av1_frames(&message)?;
            for frame in &frames.frames {
                let pts_matches = frame.pts == pts as i64;
                if measured {
                    output_frames += 1;
                    output_bytes += frame.data.len() as u64;
                    if !pts_matches {
                        pts_mismatches += 1;
                    }
                }
                packets.push(EncodedPacket {
                    input_index: index,
                    pts: frame.pts,
                    key: frame.key,
                    data: frame.data.to_vec(),
                });
            }
            if measured {
                samples.push(elapsed);
            }
        }
        drop(encoder);

        Ok(BenchmarkResult {
            encoder: kind.name(),
            initialization,
            first_frame,
            samples,
            output_frames,
            output_bytes,
            pts_mismatches,
            decoded_frames: 0,
            quality: QualityMetrics::default(),
            fps: args.fps,
            reference_format,
            packets,
        })
    }

    fn evaluate_quality(
        result: &mut BenchmarkResult,
        args: Args,
        benchmark_input: &BenchmarkInput,
    ) -> ResultType<()> {
        println!("Evaluating {} decoded quality...", result.encoder);
        let mut decoder = AomDecoder::new()?;
        let mut quality_input = benchmark_input.frame_source(result.reference_format.clone())?;
        for packet in &result.packets {
            let expected_pts = packet.input_index.saturating_mul(1000) / args.fps as usize;
            let measured = packet.input_index >= args.warmup;
            let reference = quality_input.frame(packet.input_index)?;
            let decoded = decode_packet(
                &mut decoder,
                &packet.data,
                reference,
                &result.reference_format,
                measured && packet.pts == expected_pts as i64,
                &mut result.quality,
            )?;
            if measured {
                result.decoded_frames += decoded;
            }
        }
        Ok(())
    }

    fn write_recordings(results: [&BenchmarkResult; 2], args: Args) -> ResultType<()> {
        const RECORDING_DIR: &str = "target/av1-benchmark-recordings";

        println!("Writing AV1 benchmark recordings...");
        let mut recordings = Vec::with_capacity(results.len());
        for result in results {
            let (tx, rx) = mpsc::channel();
            let mut recorder = Recorder::new(RecorderContext {
                server: false,
                id: format!("benchmark-{}", result.encoder),
                dir: RECORDING_DIR.to_owned(),
                display_idx: 0,
                camera: false,
                tx: Some(tx),
            })?;
            let frames = result
                .packets
                .iter()
                .map(|packet| EncodedVideoFrame {
                    data: Bytes::from(packet.data.clone()),
                    key: packet.key,
                    pts: packet.pts,
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            let encoded = EncodedVideoFrames {
                frames: frames.into(),
                ..Default::default()
            };
            recorder.write_frame(
                &video_frame::Union::Av1s(encoded),
                args.width as usize,
                args.height as usize,
            )?;
            let filename = match rx.recv() {
                Ok(RecordState::NewFile(filename)) => filename,
                Ok(_) => bail!("recorder did not report its output filename"),
                Err(error) => bail!("failed to receive recorder output filename: {error}"),
            };
            recordings.push((recorder, filename));
        }

        // Recorder removes files whose writer exists for less than one second.
        // Keep both muxers alive long enough for short benchmark runs as well.
        thread::sleep(Duration::from_millis(1100));
        for (_, filename) in &recordings {
            println!("Recording: {filename}");
        }
        drop(recordings);
        Ok(())
    }

    fn decode_packet(
        decoder: &mut AomDecoder,
        packet: &[u8],
        reference: &[u8],
        reference_format: &EncodeYuvFormat,
        measure_quality: bool,
        quality: &mut QualityMetrics,
    ) -> ResultType<usize> {
        let mut decoded_frames = 0usize;
        for image in decoder.decode(packet)? {
            if measure_quality {
                quality.add_frame(reference, reference_format, &image)?;
            }
            decoded_frames += 1;
        }
        for image in decoder.flush()? {
            if measure_quality {
                quality.add_frame(reference, reference_format, &image)?;
            }
            decoded_frames += 1;
        }
        if decoded_frames != 1 {
            bail!("one AV1 packet decoded to {decoded_frames} frames instead of one");
        }
        Ok(decoded_frames)
    }

    fn av1_frames(
        message: &VideoFrame,
    ) -> ResultType<&hbb_common::message_proto::EncodedVideoFrames> {
        match message.union.as_ref() {
            Some(video_frame::Union::Av1s(frames)) => Ok(frames),
            _ => bail!("encoder returned a non-AV1 VideoFrame"),
        }
    }

    fn print_results(aom: &BenchmarkResult, svt: &BenchmarkResult) {
        println!(
            "{:<9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>11} {:>11} {:>9}",
            "encoder",
            "init ms",
            "cold ms",
            "mean ms",
            "p50 ms",
            "p95 ms",
            "p99 ms",
            "encode fps",
            "avg bytes",
            "kbps"
        );
        for result in [aom, svt] {
            println!(
                "{:<9} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>11.2} {:>11.1} {:>9.1}",
                result.encoder,
                result.initialization_ms(),
                result.first_frame_ms(),
                result.mean_ms(),
                result.percentile_ms(0.50),
                result.percentile_ms(0.95),
                result.percentile_ms(0.99),
                result.encode_fps(),
                result.average_bytes(),
                result.bitrate_kbps()
            );
            println!(
                "RESULT encoder={} init_ms={:.3} cold_ms={:.3} mean_ms={:.3} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} encode_fps={:.2} output_frames={} decoded_frames={} avg_bytes={:.1} bitrate_kbps={:.1} pts_mismatches={} quality_frames={} psnr_y={:.4} psnr_u={:.4} psnr_v={:.4} psnr_yuv={:.4} ssim_y={:.6}",
                result.encoder,
                result.initialization_ms(),
                result.first_frame_ms(),
                result.mean_ms(),
                result.percentile_ms(0.50),
                result.percentile_ms(0.95),
                result.percentile_ms(0.99),
                result.encode_fps(),
                result.output_frames,
                result.decoded_frames,
                result.average_bytes(),
                result.bitrate_kbps(),
                result.pts_mismatches,
                result.quality.frames,
                result.quality.y.psnr(),
                result.quality.u.psnr(),
                result.quality.v.psnr(),
                result.quality.psnr_yuv(),
                result.quality.ssim_y()
            );
        }

        println!(
            "\n{:<9} {:>11} {:>11} {:>11} {:>11} {:>11}",
            "encoder", "PSNR-Y", "PSNR-U", "PSNR-V", "PSNR-YUV", "SSIM-Y"
        );
        for result in [aom, svt] {
            println!(
                "{:<9} {:>11.4} {:>11.4} {:>11.4} {:>11.4} {:>11.6}",
                result.encoder,
                result.quality.y.psnr(),
                result.quality.u.psnr(),
                result.quality.v.psnr(),
                result.quality.psnr_yuv(),
                result.quality.ssim_y()
            );
        }

        println!(
            "\nSVT-AV1 vs AOM: throughput={:.2}x, mean_time={:.1}%, output_size={:.1}%",
            svt.encode_fps() / aom.encode_fps(),
            svt.mean_ms() / aom.mean_ms() * 100.0,
            svt.average_bytes() / aom.average_bytes() * 100.0
        );
        println!(
            "Quality delta (SVT-AV1 - AOM): PSNR-Y={:+.4} dB, PSNR-YUV={:+.4} dB, SSIM-Y={:+.6}",
            svt.quality.y.psnr() - aom.quality.y.psnr(),
            svt.quality.psnr_yuv() - aom.quality.psnr_yuv(),
            svt.quality.ssim_y() - aom.quality.ssim_y()
        );
        println!(
            "Frame mapping: AOM input/output/decoded={}/{}/{}, PTS mismatches={}; SVT-AV1 input/output/decoded={}/{}/{}, PTS mismatches={}",
            aom.samples.len(),
            aom.output_frames,
            aom.decoded_frames,
            aom.pts_mismatches,
            svt.samples.len(),
            svt.output_frames,
            svt.decoded_frames,
            svt.pts_mismatches
        );
        println!("Higher PSNR/SSIM and throughput are better; lower time/output_size are better.");
    }

    fn parse_args() -> ResultType<Option<Args>> {
        let mut parsed = Args::default();
        let mut args = env::args().skip(1);
        while let Some(argument) = args.next() {
            if argument == "-h" || argument == "--help" {
                return Ok(None);
            }
            if argument == "--svt-first" {
                parsed.svt_first = true;
                continue;
            }
            if argument == "--record" {
                parsed.record = true;
                continue;
            }

            let (name, inline_value) = match argument.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (argument.as_str(), None),
            };
            let value = match inline_value {
                Some(value) => value.to_owned(),
                None => args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for {name}"))?,
            };
            match name {
                "--input" => parsed.input = parse_value(name, &value)?,
                "--capture-delay" => parsed.capture_delay = parse_value(name, &value)?,
                "--width" => parsed.width = parse_value(name, &value)?,
                "--height" => parsed.height = parse_value(name, &value)?,
                "--frames" => parsed.frames = parse_value(name, &value)?,
                "--warmup" => parsed.warmup = parse_value(name, &value)?,
                "--fps" => parsed.fps = parse_value(name, &value)?,
                "--quality" => parsed.quality = parse_value(name, &value)?,
                "--svt-preset" => parsed.svt_preset = parse_value(name, &value)?,
                _ => bail!("unknown option {name}\n\n{USAGE}"),
            }
        }
        Ok(Some(parsed))
    }

    fn parse_value<T>(name: &str, value: &str) -> ResultType<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        value
            .parse()
            .map_err(|error| anyhow!("invalid value for {name}: {value:?} ({error})"))
    }

    fn duration_ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1000.0
    }
}
