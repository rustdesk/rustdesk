use std::{
    sync::Arc,
    ops::Deref,
    time::Duration,
    collections::VecDeque,
    thread::{JoinHandle, self},
    ffi::c_void, io::Write
};

use hbb_common::{bail, ResultType};
use ndk::{
    media::{
        media_codec::{
            MediaCodec, MediaCodecDirection, MediaFormat,
        },
        image_reader::{
            ImageReader, ImageFormat, Image
        }
    },
    hardware_buffer::HardwareBufferUsage
};

use crate::{
    fmt_e, fmt_err, CodecFormat, NV12ToARGB, I420ToARGB,
};

use super::{StrResult, RelaxedAtomic};

use parking_lot::{Condvar, Mutex};


struct FakeThreadSafe<T>(T);
unsafe impl<T> Send for FakeThreadSafe<T> {}
unsafe impl<T> Sync for FakeThreadSafe<T> {}

impl<T> Deref for FakeThreadSafe<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

type SharedMediaCodec = Arc<FakeThreadSafe<MediaCodec>>;

pub fn configure_media_format(name: &str) -> MediaFormat {
    let media_format = MediaFormat::new();
    media_format.set_str("mime", name);
    media_format.set_i32("width", 1);
    media_format.set_i32("height", 1);
    media_format.set_i32("color-format", 2135033992);
    // 部分机型支持
    // media_format.set_i32("low-latency", 1);

    media_format
}

pub struct VideoDecoderEnqueuer {
    inner: Arc<Mutex<Option<SharedMediaCodec>>>,
}

unsafe impl Send for VideoDecoderEnqueuer {}

impl VideoDecoderEnqueuer {
    // Block until the buffer has been written or timeout is reached. Returns false if timeout.
    pub fn push_frame_nal(&self, timestamp: Duration, flag: u32, data: &[u8]) -> ResultType<bool> {
        let Some(decoder) = &*self.inner.lock() else {
            // This might happen only during destruction
            return Ok(false);
        };

        match decoder.dequeue_input_buffer(Duration::from_millis(10)) {
            Ok(Some(mut input_buffer)) => {
                input_buffer.buffer_mut()[..data.len()].copy_from_slice(data);
                decoder.queue_input_buffer(input_buffer, 0, data.len(), timestamp.as_nanos() as _, flag)
                .map_err(|e|log::debug!("At {}:{}: {e}", file!(), line!())).ok();
                Ok(true)
            }
            Ok(None) => {
                log::debug!("Failed to dequeue_input_buffer: No available input_buffer");
                Ok(true)
            }
            Err(err) => {
                log::debug!("{err}");
                Ok(false)
            }
        }
    }
}

struct QueuedImage {
    frame_image: FrameImage,
    in_use: bool,
}
unsafe impl Send for QueuedImage {}

// Access the image queue synchronously.
pub struct VideoDecoderDequeuer {
    running: Arc<RelaxedAtomic>,
    dequeue_thread: Option<JoinHandle<()>>,
    image_queue: Arc<Mutex<VecDeque<QueuedImage>>>,
    buffering_running_average: f32,
    max_buffering_frames: f32,
    buffering_history_weight: f32,
}

unsafe impl Send for VideoDecoderDequeuer {}

impl VideoDecoderDequeuer {
    // The application MUST finish using the returned buffer before calling this function again
    pub fn dequeue_frame(&mut self) -> Option<FrameImage> {
        let mut image_queue_lock = self.image_queue.lock();

        if let Some(queued_image) = image_queue_lock.front() {
            if queued_image.in_use {
                // image is released and ready to be reused by the decoder
                image_queue_lock.pop_front();
            }
        }

        // use running average to give more weight to recent samples
        self.buffering_running_average = self.buffering_running_average
            * self.buffering_history_weight
            + image_queue_lock.len() as f32 * (1. - self.buffering_history_weight);
        if self.buffering_running_average > self.max_buffering_frames as f32 {
            image_queue_lock.pop_front();
        }

        if let Some(queued_image) = image_queue_lock.front_mut() {
            queued_image.in_use = true;

            Some(
                queued_image.frame_image.clone()
            )
        } else {
            // TODO: add back when implementing proper phase sync
            //warn!("Video frame queue underflow!");
            None
        }
    }
}

impl Drop for VideoDecoderDequeuer {
    fn drop(&mut self) {
        self.running.set(false);

        // Destruction of decoder, buffered images and ImageReader
        self.dequeue_thread.take().map(|t| t.join());
    }
}

pub fn video_decoder_split(
    name: &CodecFormat,
    direction: MediaCodecDirection,
) -> StrResult<(VideoDecoderEnqueuer, VideoDecoderDequeuer)> {
    let running = Arc::new(RelaxedAtomic::new(true));
    let decoder_enqueuer = Arc::new(Mutex::new(None::<SharedMediaCodec>));
    let decoder_ready_notifier = Arc::new(Condvar::new());
    let image_queue = Arc::new(Mutex::new(VecDeque::<QueuedImage>::new()));

    let max_buffering_frames = 1.5;
    let buffering_history_weight=0.90;
    let _name = Arc::new(name.to_mime_type());

    let dequeue_thread = thread::spawn({

        let running = Arc::clone(&running);
        let decoder_enqueuer = Arc::clone(&decoder_enqueuer);
        let decoder_ready_notifier = Arc::clone(&decoder_ready_notifier);
        let image_queue = Arc::clone(&image_queue);

        move || {
            const MAX_BUFFERING_FRAMES: usize = 10;
            let name = *_name;

            // 2x: keep the target buffering in the middle of the max amount of queuable frames
            let available_buffering_frames = (2. * max_buffering_frames as f64).ceil() as usize;
            // log::debug!("available_buffering_frames:{:?}", available_buffering_frames);

            let format = configure_media_format(name);

            let mut image_reader = ImageReader::new_with_usage(
                1,
                1,
                // ndk::media::image_reader::ImageFormat::RGBA_8888,
                ImageFormat::YUV_420_888,
                // HardwareBufferUsage::GPU_SAMPLED_IMAGE,
                HardwareBufferUsage::CPU_READ_RARELY,
                // HardwareBufferUsage::CPU_READ_OFTEN,
                MAX_BUFFERING_FRAMES as i32,
            )
            .unwrap();

            image_reader
                .set_image_listener(Box::new({
                    let image_queue = Arc::clone(&image_queue);
                    move |image_reader| {
                        let mut image_queue_lock = image_queue.lock();

                        if image_queue_lock.len() > available_buffering_frames {
                            log::warn!("Video frame queue overflow!");
                            image_queue_lock.pop_front();
                        }

                        match &mut image_reader.acquire_next_image() {
                            Ok(image @ Some(_)) => {
                                let image = image.take().unwrap();
                                // let timestamp =
                                //     Duration::from_nanos(image.get_timestamp().unwrap() as u64);

                                let frame_image = unsafe { FrameImage::new(&image) };
                                drop(image);

                                image_queue_lock.push_back(QueuedImage {
                                    frame_image,
                                    in_use: false,
                                });
                            }
                            Ok(None) => {
                                log::error!("ImageReader error: No buffer available");

                                image_queue_lock.clear();
                            }
                            Err(e) => {
                                log::error!("ImageReader error: {e}");

                                image_queue_lock.clear();
                            }
                        }
                    }
                }))
                .unwrap();

            // Documentation says that this call is necessary to properly dispose acquired buffers.
            // todo: find out how to use it and avoid leaking the ImageReader
            image_reader
                .set_buffer_removed_listener(Box::new(|_, _| ()))
                .unwrap();

            let decoder = Arc::new(FakeThreadSafe(
                MediaCodec::from_decoder_type(&name).unwrap(),
            ));
            decoder
                .configure(
                    &format,
                    Some(&image_reader.get_window().unwrap()),
                    // MediaCodecDirection::Decoder,
                    direction,
                )
                .unwrap();
            decoder.start().unwrap();

            {
                let mut decoder_lock = decoder_enqueuer.lock();

                *decoder_lock = Some(Arc::clone(&decoder));

                decoder_ready_notifier.notify_one();
            }

            while running.value() {
                match decoder.dequeue_output_buffer(Duration::from_millis(100)) {
                    Ok(Some(output_buffer)) => {
                        let presentation_time_ns = output_buffer.presentation_time_us();
                        if let Err(e) =
                        decoder.release_output_buffer_at_time(output_buffer, presentation_time_ns)
                        {
                            log::error!("Decoder dequeue error: {e}");
                        }
                    },
                    Ok(None) => thread::yield_now(),
                    Err(err) => {
                        log::debug!("{err}");
                    },
                }
            }

            // Destroy all resources
            decoder_enqueuer.lock().take(); // Make sure the shared ref is deleted first
            decoder.stop().unwrap();
            drop(decoder);

            image_queue.lock().clear();
            log::error!("FIXME: Leaking Imagereader!");
            Box::leak(Box::new(image_reader));
        }
    });

    // Make sure the decoder is ready: we don't want to try to enqueue frame and lose them, to avoid
    // image corruption.
    {
        let mut decoder_lock = decoder_enqueuer.lock();

        if decoder_lock.is_none() {
            // No spurious wakeups
            decoder_ready_notifier.wait(&mut decoder_lock);
        }
    }

    let enqueuer = VideoDecoderEnqueuer {
        inner: decoder_enqueuer,
    };
    let dequeuer = VideoDecoderDequeuer {
        running,
        dequeue_thread: Some(dequeue_thread),
        image_queue,
        max_buffering_frames,
        buffering_history_weight,
        buffering_running_average: 0.1,
    };

    Ok((enqueuer, dequeuer))
}


#[derive(Debug, Clone)]
pub struct FrameImage {
    inner: Vec<u8>,
    w: usize,
    h: usize,
    pixel_stride_uv: usize,
    timestamp : i64,
}

impl FrameImage {
    pub unsafe fn new(image: &Image) -> FrameImage {
        let timestamp = image.get_timestamp().unwrap();
        let width = image.get_width().unwrap();
        let height = image.get_height().unwrap();
        let plane_count = image.get_number_of_planes().unwrap();
        // log::debug!("AHardwareBuffer: {:?} ({}x{} {:?}) ",hw_buffer.as_ptr(),width,height,format);

        let mut pixel_stride_uv = 0;
        let mut i420_vec = Vec::new();
        for i in 0..plane_count {
            let plane_data = image.get_plane_data(i).unwrap();
            let pixel_stride = image.get_plane_pixel_stride(i).unwrap() as usize;
            let row_stride = image.get_plane_row_stride(i).unwrap() as usize;
            let w = match i {
                0 => width,
                _ => width / 2,
            } as usize;
            let h = match i {
                0 => height,
                _ => height / 2,
            } as usize;
            let row_bytes = pixel_stride * w;
            let src = plane_data;
            let mut row_start_index = 0;
            for _ in 0..h {
                let row_end_index = row_start_index + row_bytes;
                let row = &src[row_start_index..row_end_index];
                i420_vec.extend_from_slice(row);
                row_start_index += row_stride;
            }
            if pixel_stride_uv != pixel_stride {
                pixel_stride_uv = pixel_stride;
            }
        }
        FrameImage {
            inner: i420_vec,
            w: width as _,
            h: height as _,
            pixel_stride_uv,
            timestamp,
        }
    }

    pub fn i420_to_argb<'a>(&'a mut self, i420: &'a mut Vec<u8>) {
        conv_i420_to_argb(&mut self.inner, i420, self.pixel_stride_uv, self.w, self.h);
    }

    pub fn i420_to_abgr<'a>(&'a mut self, i420: &'a mut Vec<u8>) {
        todo!();
    }

}

fn conv_i420_to_argb(src: &mut Vec<u8>, dst: &mut Vec<u8>, pixel_uv: usize, w: usize, h: usize) {

        let bps = 4;
        let stride = w as i32;
        let y_ptr = src.as_ptr();
        let u = src.len() * 4 / 6;
        let v = src.len() * 5 / 6;
        let u_ptr = src[u..].as_ptr();
        let v_ptr = src[v..].as_ptr();
        let uv= src.len() /2 ;
        let uv_ptr = src[uv..].as_ptr();

        dst.resize((h * w * bps) as usize, 0);

        unsafe {
        match pixel_uv {
            1 => {
                I420ToARGB(
                    y_ptr,
                    stride,
                    u_ptr,
                    stride / 2 ,
                    v_ptr,
                    stride / 2 ,
                    dst.as_mut_ptr(),
                    (w * bps) as _,
                    w as _,
                    h as _,
                );
            }
            2 => {
                NV12ToARGB(
                    y_ptr,
                    stride,
                    uv_ptr,
                    stride,
                    dst.as_mut_ptr(),
                    (w * bps) as _,
                    w as _,
                    h as _,
                );
            }
            _ => {
                log::debug!("Unsupported image format");
            }
        }
    }
}
