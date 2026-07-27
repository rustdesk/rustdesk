use super::{Pixfmt, TraitPixelBuffer};

pub mod avcodec;
pub mod direct_render;
pub use direct_render::{
    lookup_direct_render_target, register_direct_render_target_lookup, register_render_context,
    register_render_stats_callback, DirectRenderTarget, DirectRenderTargetLookup,
    RenderStatsCallback,
};
pub struct PixelBuffer<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
    stride: Vec<usize>,
    pixfmt: Pixfmt,
}

impl<'a> PixelBuffer<'a> {
    pub fn new(
        data: &'a [u8],
        width: usize,
        height: usize,
        stride: Vec<usize>,
        pixfmt: Pixfmt,
    ) -> Self {
        Self {
            data,
            width,
            height,
            stride,
            pixfmt,
        }
    }
}

impl<'a> TraitPixelBuffer for PixelBuffer<'a> {
    fn data(&self) -> &[u8] {
        self.data
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn stride(&self) -> Vec<usize> {
        self.stride.clone()
    }

    fn pixfmt(&self) -> Pixfmt {
        self.pixfmt
    }
}
