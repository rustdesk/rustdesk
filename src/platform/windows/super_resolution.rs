use std::{ffi::c_void, mem::ManuallyDrop};
use windows::{
    core::{Error, Interface, GUID, HRESULT},
    Win32::{
        Foundation::{E_FAIL, RECT, TRUE},
        Graphics::{
            Direct3D11::*,
            Dxgi::{Common::*, IDXGIDevice},
        },
    },
};

const NVIDIA_VENDOR_ID: u32 = 0x10de;
const NVIDIA_PPE_STREAM_VERSION_V1: u32 = 1;
const NVIDIA_PPE_METHOD_SUPER_RESOLUTION: u32 = 2;
// Vendor-private stream extension ABI used by Chromium's SwapChainPresenter.
const NVIDIA_PPE_INTERFACE: GUID = GUID::from_values(
    0xd43ce1b3,
    0x1f4b,
    0x48ac,
    [0xba, 0xee, 0xc3, 0xc2, 0x53, 0x75, 0xe6, 0xf7],
);

#[repr(C)]
struct NvidiaPpeStreamExtension {
    version: u32,
    method: u32,
    enable: u32,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct ConfigKey {
    device: usize,
    input_width: u32,
    input_height: u32,
    output_width: u32,
    output_height: u32,
    format: i32,
}

struct Pipeline {
    context: ID3D11DeviceContext,
    video_device: ID3D11VideoDevice,
    video_context: ID3D11VideoContext1,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    output: ID3D11Texture2D,
    output_view: ID3D11VideoProcessorOutputView,
}

impl Pipeline {
    unsafe fn new(
        device: &ID3D11Device,
        input_desc: &D3D11_TEXTURE2D_DESC,
        output_width: u32,
        output_height: u32,
    ) -> windows::core::Result<Self> {
        let context = device.GetImmediateContext()?;
        let video_device = device.cast::<ID3D11VideoDevice>()?;
        let video_context = context.cast::<ID3D11VideoContext1>()?;
        let content_desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            InputWidth: input_desc.Width,
            InputHeight: input_desc.Height,
            OutputFrameRate: DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            OutputWidth: output_width,
            OutputHeight: output_height,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        let enumerator = video_device.CreateVideoProcessorEnumerator(&content_desc)?;
        let processor = video_device.CreateVideoProcessor(&enumerator, 0)?;

        let mut output_desc = *input_desc;
        output_desc.Width = output_width;
        output_desc.Height = output_height;
        output_desc.Usage = D3D11_USAGE_DEFAULT;
        output_desc.BindFlags = (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32;
        output_desc.CPUAccessFlags = 0;
        output_desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED.0 as u32;
        let mut output = None;
        device.CreateTexture2D(&output_desc, None, Some(&mut output))?;
        let output = output.ok_or_else(|| Error::from(HRESULT(E_FAIL.0)))?;
        let mut output_view_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC::default();
        output_view_desc.ViewDimension = D3D11_VPOV_DIMENSION_TEXTURE2D;
        output_view_desc.Anonymous.Texture2D.MipSlice = 0;
        let mut output_view = None;
        video_device.CreateVideoProcessorOutputView(
            &output,
            &enumerator,
            &output_view_desc,
            Some(&mut output_view),
        )?;
        let output_view = output_view.ok_or_else(|| Error::from(HRESULT(E_FAIL.0)))?;

        let source_rect = RECT {
            left: 0,
            top: 0,
            right: input_desc.Width as i32,
            bottom: input_desc.Height as i32,
        };
        let output_rect = RECT {
            left: 0,
            top: 0,
            right: output_width as i32,
            bottom: output_height as i32,
        };
        video_context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
        video_context.VideoProcessorSetStreamFrameFormat(
            &processor,
            0,
            D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
        );
        video_context.VideoProcessorSetStreamColorSpace1(
            &processor,
            0,
            DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
        );
        video_context.VideoProcessorSetOutputColorSpace1(
            &processor,
            DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
        );
        video_context.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&source_rect));
        video_context.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&output_rect));
        video_context.VideoProcessorSetOutputTargetRect(&processor, true, Some(&output_rect));

        let extension = NvidiaPpeStreamExtension {
            version: NVIDIA_PPE_STREAM_VERSION_V1,
            method: NVIDIA_PPE_METHOD_SUPER_RESOLUTION,
            enable: 1,
        };
        HRESULT(video_context.VideoProcessorSetStreamExtension(
            &processor,
            0,
            &NVIDIA_PPE_INTERFACE,
            std::mem::size_of::<NvidiaPpeStreamExtension>() as u32,
            &extension as *const _ as *const c_void,
        ))
        .ok()?;

        Ok(Self {
            context,
            video_device,
            video_context,
            enumerator,
            processor,
            output,
            output_view,
        })
    }

    unsafe fn blit(&self, input: &ID3D11Texture2D) -> windows::core::Result<()> {
        let mut input_view_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC::default();
        input_view_desc.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D;
        input_view_desc.Anonymous.Texture2D.MipSlice = 0;
        input_view_desc.Anonymous.Texture2D.ArraySlice = 0;
        let mut input_view = None;
        self.video_device.CreateVideoProcessorInputView(
            input,
            &self.enumerator,
            &input_view_desc,
            Some(&mut input_view),
        )?;
        let input_view = input_view.ok_or_else(|| Error::from(HRESULT(E_FAIL.0)))?;
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: TRUE,
            pInputSurface: ManuallyDrop::new(Some(input_view)),
            ..Default::default()
        };
        let result = self.video_context.VideoProcessorBlt(
            &self.processor,
            &self.output_view,
            0,
            std::slice::from_ref(&stream),
        );
        ManuallyDrop::drop(&mut stream.pInputSurface);
        result?;
        self.context.Flush();
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct PpeScaler {
    target: (u32, u32),
    key: Option<ConfigKey>,
    pipeline: Option<Pipeline>,
    failed: bool,
}

impl PpeScaler {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        let mut scaler = Self::default();
        scaler.set_target(width, height);
        scaler
    }

    pub(crate) fn set_target(&mut self, width: u32, height: u32) {
        let target = (width, height);
        if self.target != target {
            self.target = target;
            self.key = None;
            self.pipeline = None;
            self.failed = false;
        }
    }

    pub(crate) fn process(&mut self, texture: *mut c_void) -> *mut c_void {
        if texture.is_null() || self.target.0 == 0 || self.target.1 == 0 {
            return texture;
        }
        match unsafe { self.try_process(texture) } {
            Ok(Some(output)) => output,
            Ok(None) => texture,
            Err(e) => {
                self.pipeline = None;
                self.failed = true;
                hbb_common::throttled_log!(
                    std::time::Duration::from_secs(30),
                    warn,
                    "NVIDIA PPE super resolution disabled for this configuration: {e}"
                );
                texture
            }
        }
    }

    unsafe fn try_process(
        &mut self,
        texture: *mut c_void,
    ) -> windows::core::Result<Option<*mut c_void>> {
        let Some(input) = ID3D11Texture2D::from_raw_borrowed(&texture) else {
            return Ok(None);
        };
        let mut input_desc = D3D11_TEXTURE2D_DESC::default();
        input.GetDesc(&mut input_desc);
        if input_desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || input_desc.ArraySize != 1
            || self.target.0 <= input_desc.Width
            || self.target.1 <= input_desc.Height
            || self.target.0 > D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION
            || self.target.1 > D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION
        {
            self.key = None;
            self.pipeline = None;
            self.failed = false;
            return Ok(None);
        }

        let device = input.GetDevice()?;
        let key = ConfigKey {
            device: device.as_raw() as usize,
            input_width: input_desc.Width,
            input_height: input_desc.Height,
            output_width: self.target.0,
            output_height: self.target.1,
            format: input_desc.Format.0,
        };
        if self.key != Some(key) {
            self.key = Some(key);
            self.pipeline = None;
            self.failed = false;
        }
        if self.failed {
            return Ok(None);
        }

        let dxgi_device = device.cast::<IDXGIDevice>()?;
        let adapter = dxgi_device.GetAdapter()?;
        if adapter.GetDesc()?.VendorId != NVIDIA_VENDOR_ID {
            self.failed = true;
            return Ok(None);
        }
        if self.pipeline.is_none() {
            let pipeline = Pipeline::new(&device, &input_desc, self.target.0, self.target.1)?;
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(30),
                info,
                "NVIDIA PPE super resolution enabled: {}x{} -> {}x{}",
                input_desc.Width,
                input_desc.Height,
                self.target.0,
                self.target.1
            );
            self.pipeline = Some(pipeline);
        }
        let Some(pipeline) = &self.pipeline else {
            return Ok(None);
        };
        pipeline.blit(input)?;
        Ok(Some(pipeline.output.as_raw()))
    }
}
