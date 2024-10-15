use crate::CodecFormat;
#[cfg(feature = "hwcodec")]
use hbb_common::anyhow::anyhow;
use hbb_common::{
    bail, chrono, log,
    message_proto::{message, video_frame, EncodedVideoFrame, Message},
    ResultType,
};
#[cfg(feature = "hwcodec")]
use hwcodec::mux::{MuxContext, Muxer};
use std::{
    fs::{File, OpenOptions},
    io,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::mpsc::Sender,
    time::Instant,
};
use webm::mux::{self, Segment, Track, VideoTrack, Writer};

const MIN_SECS: u64 = 1;

#[derive(Debug, Clone)]
pub struct RecorderContext {
    pub server: bool,
    pub id: String,
    pub dir: String,
    pub filename: String,
    pub width: usize,
    pub height: usize,
    pub format: CodecFormat,
    pub tx: Option<Sender<RecordState>>,
}

impl RecorderContext {
    pub fn set_filename(&mut self) -> ResultType<()> {
        if !PathBuf::from(&self.dir).exists() {
            std::fs::create_dir_all(&self.dir)?;
        }
        let file = if self.server { "incoming" } else { "outgoing" }.to_string()
            + "_"
            + &self.id.clone()
            + &chrono::Local::now().format("_%Y%m%d%H%M%S%3f_").to_string()
            + &self.format.to_string().to_lowercase()
            + if self.format == CodecFormat::VP9
                || self.format == CodecFormat::VP8
                || self.format == CodecFormat::AV1
            {
                ".webm"
            } else {
                ".mp4"
            };
        self.filename = PathBuf::from(&self.dir)
            .join(file)
            .to_string_lossy()
            .to_string();
        log::info!("video will save to {}", self.filename);
        Ok(())
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

#[derive(Debug)]
pub enum RecordState {
    NewFile(String),
    NewFrame,
    WriteTail,
    RemoveFile,
}

pub struct Recorder {
    pub inner: Box<dyn RecorderApi>,
    ctx: RecorderContext,
    pts: Option<i64>,
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
        let recorder = match ctx.format {
            CodecFormat::VP8 | CodecFormat::VP9 | CodecFormat::AV1 => Recorder {
                inner: Box::new(WebmRecorder::new(ctx.clone())?),
                ctx,
                pts: None,
            },
            #[cfg(feature = "hwcodec")]
            _ => Recorder {
                inner: Box::new(HwRecorder::new(ctx.clone())?),
                ctx,
                pts: None,
            },
            #[cfg(not(feature = "hwcodec"))]
            _ => bail!("unsupported codec type"),
        };
        recorder.send_state(RecordState::NewFile(recorder.ctx.filename.clone()));
        Ok(recorder)
    }

    fn change(&mut self, mut ctx: RecorderContext) -> ResultType<()> {
        ctx.set_filename()?;
        self.inner = match ctx.format {
            CodecFormat::VP8 | CodecFormat::VP9 | CodecFormat::AV1 => {
                Box::new(WebmRecorder::new(ctx.clone())?)
            }
            #[cfg(feature = "hwcodec")]
            _ => Box::new(HwRecorder::new(ctx.clone())?),
            #[cfg(not(feature = "hwcodec"))]
            _ => bail!("unsupported codec type"),
        };
        self.ctx = ctx;
        self.pts = None;
        self.send_state(RecordState::NewFile(self.ctx.filename.clone()));
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
            video_frame::Union::Vp8s(vp8s) => {
                if self.ctx.format != CodecFormat::VP8 {
                    self.change(RecorderContext {
                        format: CodecFormat::VP8,
                        ..self.ctx.clone()
                    })?;
                }
                for f in vp8s.frames.iter() {
                    self.check_pts(f.pts)?;
                    self.write_video(f);
                }
            }
            video_frame::Union::Vp9s(vp9s) => {
                if self.ctx.format != CodecFormat::VP9 {
                    self.change(RecorderContext {
                        format: CodecFormat::VP9,
                        ..self.ctx.clone()
                    })?;
                }
                for f in vp9s.frames.iter() {
                    self.check_pts(f.pts)?;
                    self.write_video(f);
                }
            }
            video_frame::Union::Av1s(av1s) => {
                if self.ctx.format != CodecFormat::AV1 {
                    self.change(RecorderContext {
                        format: CodecFormat::AV1,
                        ..self.ctx.clone()
                    })?;
                }
                for f in av1s.frames.iter() {
                    self.check_pts(f.pts)?;
                    self.write_video(f);
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H264s(h264s) => {
                if self.ctx.format != CodecFormat::H264 {
                    self.change(RecorderContext {
                        format: CodecFormat::H264,
                        ..self.ctx.clone()
                    })?;
                }
                for f in h264s.frames.iter() {
                    self.check_pts(f.pts)?;
                    self.write_video(f);
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H265s(h265s) => {
                if self.ctx.format != CodecFormat::H265 {
                    self.change(RecorderContext {
                        format: CodecFormat::H265,
                        ..self.ctx.clone()
                    })?;
                }
                for f in h265s.frames.iter() {
                    self.check_pts(f.pts)?;
                    self.write_video(f);
                }
            }
            _ => bail!("unsupported frame type"),
        }
        self.send_state(RecordState::NewFrame);
        Ok(())
    }

    fn check_pts(&mut self, pts: i64) -> ResultType<()> {
        // https://stackoverflow.com/questions/76379101/how-to-create-one-playable-webm-file-from-two-different-video-tracks-with-same-c
        let old_pts = self.pts;
        self.pts = Some(pts);
        if old_pts.clone().unwrap_or_default() > pts {
            log::info!("pts {:?} -> {}, change record filename", old_pts, pts);
            self.change(self.ctx.clone())?;
        }
        Ok(())
    }

    fn send_state(&self, state: RecordState) {
        self.ctx.tx.as_ref().map(|tx| tx.send(state));
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
            if ctx.format == CodecFormat::VP9 {
                mux::VideoCodecId::VP9
            } else if ctx.format == CodecFormat::VP8 {
                mux::VideoCodecId::VP8
            } else {
                mux::VideoCodecId::AV1
            },
        );
        if ctx.format == CodecFormat::AV1 {
            // [129, 8, 12, 0] in 3.6.0, but zero works
            let codec_private = vec![0, 0, 0, 0];
            if !webm.set_codec_private(vt.track_number(), &codec_private) {
                bail!("Failed to set codec private");
            }
        }
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
        let _ = std::mem::replace(&mut self.webm, None).map_or(false, |webm| webm.finalize(None));
        let mut state = RecordState::WriteTail;
        if !self.written || self.start.elapsed().as_secs() < MIN_SECS {
            std::fs::remove_file(&self.ctx.filename).ok();
            state = RecordState::RemoveFile;
        }
        self.ctx.tx.as_ref().map(|tx| tx.send(state));
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
            is265: ctx.format == CodecFormat::H265,
            framerate: crate::hwcodec::DEFAULT_FPS as _,
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
        self.muxer.write_tail().ok();
        let mut state = RecordState::WriteTail;
        if !self.written || self.start.elapsed().as_secs() < MIN_SECS {
            std::fs::remove_file(&self.ctx.filename).ok();
            state = RecordState::RemoveFile;
        }
        self.ctx.tx.as_ref().map(|tx| tx.send(state));
    }
}
