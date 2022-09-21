#[cfg(feature = "hwcodec")]
use hbb_common::anyhow::anyhow;
use hbb_common::{
    bail, chrono,
    config::Config,
    directories_next,
    message_proto::{message, video_frame, EncodedVideoFrame, Message},
    ResultType,
};
#[cfg(feature = "hwcodec")]
use hwcodec::mux::{MuxContext, Muxer};
use std::{
    fs::{File, OpenOptions},
    io,
    time::Instant,
};
use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
};
use webm::mux::{self, Segment, Track, VideoTrack, Writer};

const MIN_SECS: u64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum RecodeCodecID {
    VP9,
    H264,
    H265,
}

#[derive(Debug, Clone)]
pub struct RecorderContext {
    pub id: String,
    pub filename: String,
    pub width: usize,
    pub height: usize,
    pub codec_id: RecodeCodecID,
}

impl RecorderContext {
    pub fn set_filename(&mut self) -> ResultType<()> {
        let mut dir = Config::get_option("video-save-directory");
        if !dir.is_empty() {
            if !PathBuf::from(&dir).exists() {
                std::fs::create_dir_all(&dir)?;
            }
        } else {
            dir = Self::default_save_directory();
            if !dir.is_empty() && !PathBuf::from(&dir).exists() {
                std::fs::create_dir_all(&dir)?;
            }
        }
        let file = self.id.clone()
            + &chrono::Local::now().format("_%Y%m%d%H%M%S").to_string()
            + if self.codec_id == RecodeCodecID::VP9 {
                ".webm"
            } else {
                ".mp4"
            };
        self.filename = PathBuf::from(&dir).join(file).to_string_lossy().to_string();
        Ok(())
    }

    pub fn default_save_directory() -> String {
        if let Some(user) = directories_next::UserDirs::new() {
            if let Some(video_dir) = user.video_dir() {
                return video_dir.join("RustDesk").to_string_lossy().to_string();
            }
        }
        "".to_owned()
    }
}

unsafe impl Send for Recorder {}
unsafe impl Sync for Recorder {}

pub trait RecorderApi {
    fn new(ctx: RecorderContext) -> ResultType<Self>
    where
        Self: Sized;
    fn write_video(&mut self, frame: &EncodedVideoFrame) -> bool;
}

pub struct Recorder {
    pub inner: Box<dyn RecorderApi>,
    ctx: RecorderContext,
}

impl Deref for Recorder {
    type Target = Box<dyn RecorderApi>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Recorder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Recorder {
    pub fn new(mut ctx: RecorderContext) -> ResultType<Self> {
        ctx.set_filename()?;
        let recorder = match ctx.codec_id {
            RecodeCodecID::VP9 => Recorder {
                inner: Box::new(WebmRecorder::new(ctx.clone())?),
                ctx,
            },
            #[cfg(feature = "hwcodec")]
            _ => Recorder {
                inner: Box::new(HwRecorder::new(ctx.clone())?),
                ctx,
            },
            #[cfg(not(feature = "hwcodec"))]
            _ => bail!("unsupported codec type"),
        };
        Ok(recorder)
    }

    fn change(&mut self, mut ctx: RecorderContext) -> ResultType<()> {
        ctx.set_filename()?;
        self.inner = match ctx.codec_id {
            RecodeCodecID::VP9 => Box::new(WebmRecorder::new(ctx.clone())?),
            #[cfg(feature = "hwcodec")]
            _ => Box::new(HwRecorder::new(ctx.clone())?),
            #[cfg(not(feature = "hwcodec"))]
            _ => bail!("unsupported codec type"),
        };
        self.ctx = ctx;
        Ok(())
    }

    pub fn write_message(&mut self, msg: &Message) {
        if let Some(message::Union::VideoFrame(vf)) = &msg.union {
            if let Some(frame) = &vf.union {
                self.write_frame(frame).ok();
            }
        }
    }

    pub fn write_frame(&mut self, frame: &video_frame::Union) -> ResultType<()> {
        match frame {
            video_frame::Union::Vp9s(vp9s) => {
                if self.ctx.codec_id != RecodeCodecID::VP9 {
                    self.change(RecorderContext {
                        codec_id: RecodeCodecID::VP9,
                        ..self.ctx.clone()
                    })?;
                }
                vp9s.frames.iter().map(|f| self.write_video(f)).count();
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H264s(h264s) => {
                if self.ctx.codec_id != RecodeCodecID::H264 {
                    self.change(RecorderContext {
                        codec_id: RecodeCodecID::H264,
                        ..self.ctx.clone()
                    })?;
                }
                if self.ctx.codec_id == RecodeCodecID::H264 {
                    h264s.frames.iter().map(|f| self.write_video(f)).count();
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H265s(h265s) => {
                if self.ctx.codec_id != RecodeCodecID::H265 {
                    self.change(RecorderContext {
                        codec_id: RecodeCodecID::H265,
                        ..self.ctx.clone()
                    })?;
                }
                if self.ctx.codec_id == RecodeCodecID::H265 {
                    h265s.frames.iter().map(|f| self.write_video(f)).count();
                }
            }
            _ => bail!("unsupported frame type"),
        }
        Ok(())
    }
}

struct WebmRecorder {
    vt: VideoTrack,
    webm: Option<Segment<Writer<File>>>,
    ctx: RecorderContext,
    key: bool,
    written: bool,
    start: Instant,
}

impl RecorderApi for WebmRecorder {
    fn new(ctx: RecorderContext) -> ResultType<Self> {
        let out = match {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&ctx.filename)
        } {
            Ok(file) => file,
            Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => File::create(&ctx.filename)?,
            Err(e) => return Err(e.into()),
        };
        let mut webm = match mux::Segment::new(mux::Writer::new(out)) {
            Some(v) => v,
            None => bail!("Failed to create webm mux"),
        };
        let vt = webm.add_video_track(
            ctx.width as _,
            ctx.height as _,
            None,
            mux::VideoCodecId::VP9,
        );
        Ok(WebmRecorder {
            vt,
            webm: Some(webm),
            ctx,
            key: false,
            written: false,
            start: Instant::now(),
        })
    }

    fn write_video(&mut self, frame: &EncodedVideoFrame) -> bool {
        if frame.key {
            self.key = true;
        }
        if self.key {
            let ok = self
                .vt
                .add_frame(&frame.data, frame.pts as u64 * 1_000_000, frame.key);
            if ok {
                self.written = true;
            }
            ok
        } else {
            false
        }
    }
}

impl Drop for WebmRecorder {
    fn drop(&mut self) {
        std::mem::replace(&mut self.webm, None).map_or(false, |webm| webm.finalize(None));
        if !self.written || self.start.elapsed().as_secs() < MIN_SECS {
            std::fs::remove_file(&self.ctx.filename).ok();
        }
    }
}

#[cfg(feature = "hwcodec")]
struct HwRecorder {
    muxer: Muxer,
    ctx: RecorderContext,
    written: bool,
    key: bool,
    start: Instant,
}

#[cfg(feature = "hwcodec")]
impl RecorderApi for HwRecorder {
    fn new(ctx: RecorderContext) -> ResultType<Self> {
        let muxer = Muxer::new(MuxContext {
            filename: ctx.filename.clone(),
            width: ctx.width,
            height: ctx.height,
            is265: ctx.codec_id == RecodeCodecID::H265,
            framerate: crate::hwcodec::DEFAULT_TIME_BASE[1] as _,
        })
        .map_err(|_| anyhow!("Failed to create hardware muxer"))?;
        Ok(HwRecorder {
            muxer,
            ctx,
            written: false,
            key: false,
            start: Instant::now(),
        })
    }

    fn write_video(&mut self, frame: &EncodedVideoFrame) -> bool {
        if frame.key {
            self.key = true;
        }
        if self.key {
            let ok = self.muxer.write_video(&frame.data, frame.key).is_ok();
            if ok {
                self.written = true;
            }
            ok
        } else {
            false
        }
    }
}

#[cfg(feature = "hwcodec")]
impl Drop for HwRecorder {
    fn drop(&mut self) {
        log::info!("DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD {}", self.ctx.filename);
        self.muxer.write_tail().ok();
        if !self.written || self.start.elapsed().as_secs() < MIN_SECS {
            std::fs::remove_file(&self.ctx.filename).ok();
        }
        log::info!("DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD ok");
    }
}
