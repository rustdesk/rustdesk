use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{storage::restrict_file, SegmentPaths};
use webm::mux::{self, Segment, Track, VideoTrack, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentEvent {
    Started,
    Continue,
    Rotate,
}

#[derive(Debug)]
pub struct SegmentPlanner {
    segment_duration: Duration,
    segment_start: Option<Instant>,
    dimensions: Option<(u32, u32)>,
}

impl SegmentPlanner {
    pub fn new(segment_duration: Duration) -> Self {
        Self {
            segment_duration,
            segment_start: None,
            dimensions: None,
        }
    }

    pub fn start(&mut self, now: Instant, width: u32, height: u32) -> SegmentEvent {
        self.segment_start = Some(now);
        self.dimensions = Some((width, height));
        SegmentEvent::Started
    }

    pub fn update(&mut self, now: Instant, width: u32, height: u32) -> SegmentEvent {
        let Some(start) = self.segment_start else {
            return self.start(now, width, height);
        };

        if self.dimensions != Some((width, height))
            || now.duration_since(start) >= self.segment_duration
        {
            self.segment_start = Some(now);
            self.dimensions = Some((width, height));
            return SegmentEvent::Rotate;
        }

        SegmentEvent::Continue
    }
}

pub struct WebmSegmentWriter {
    paths: SegmentPaths,
    webm: Option<Segment<Writer<std::fs::File>>>,
    video_track: VideoTrack,
    wrote_frame: bool,
}

impl WebmSegmentWriter {
    pub fn new(paths: SegmentPaths, width: u32, height: u32) -> hbb_common::ResultType<Self> {
        let out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.temporary)?;
        restrict_file(&paths.temporary)?;
        let mut webm = mux::Segment::new(mux::Writer::new(out))
            .ok_or_else(|| anyhow::anyhow!("failed to create webm mux"))?;
        let video_track = webm.add_video_track(width, height, None, mux::VideoCodecId::VP8);
        Ok(Self {
            paths,
            webm: Some(webm),
            video_track,
            wrote_frame: false,
        })
    }

    pub fn write_encoded_frame(
        &mut self,
        data: &[u8],
        timestamp_ms: u64,
        key: bool,
    ) -> hbb_common::ResultType<()> {
        if !self
            .video_track
            .add_frame(data, timestamp_ms * 1_000_000, key)
        {
            anyhow::bail!("failed to write webm frame");
        }
        self.wrote_frame = true;
        Ok(())
    }

    pub fn finalize(mut self) -> hbb_common::ResultType<PathBuf> {
        self.finalize_inner()
    }

    pub fn finalize_to(mut self, complete_path: PathBuf) -> hbb_common::ResultType<PathBuf> {
        self.paths.complete = complete_path;
        self.finalize_inner()
    }

    fn finalize_inner(&mut self) -> hbb_common::ResultType<PathBuf> {
        if let Some(webm) = self.webm.take() {
            if !webm.finalize(None) {
                anyhow::bail!("failed to finalize webm segment");
            }
        }
        if !self.wrote_frame {
            std::fs::remove_file(&self.paths.temporary).ok();
            anyhow::bail!("cannot finalize empty webm segment");
        }
        std::fs::rename(&self.paths.temporary, &self.paths.complete)?;
        restrict_file(&self.paths.complete)?;
        Ok(self.paths.complete.clone())
    }
}

impl Drop for WebmSegmentWriter {
    fn drop(&mut self) {
        if self.webm.is_some() {
            let _ = self.webm.take().map(|webm| webm.finalize(None));
            let _ = std::fs::remove_file(&self.paths.temporary);
        }
    }
}
