use super::super::VRamEncoder;
use crate::{codec::EncoderApi, EncodeInput};
use base::message_proto::{video_frame, VideoFrame};
use hwcodec::{
    common::{DataFormat, Driver, MAX_GOP},
    ffmpeg::AVHWDeviceType,
    ffmpeg_ram::decode::{DecodeContext, Decoder},
    vram::{encode::Encoder, DynamicContext, EncodeContext, FeatureContext},
};
use std::{mem, ptr};
use winapi::{
    shared::{
        dxgi::{CreateDXGIFactory1, IDXGIFactory1},
        dxgiformat::DXGI_FORMAT_B8G8R8A8_UNORM,
        dxgitype::DXGI_SAMPLE_DESC,
        winerror::DXGI_ERROR_NOT_FOUND,
    },
    um::{d3d11::*, d3dcommon::D3D_DRIVER_TYPE_UNKNOWN, unknwnbase::IUnknown},
    Interface,
};

struct Com<T>(*mut T);

impl<T> Drop for Com<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { (*(self.0 as *mut IUnknown)).Release() };
        }
    }
}

unsafe fn texture(
    device: *mut ID3D11Device,
    width: u32,
    height: u32,
    shade: u8,
) -> Com<ID3D11Texture2D> {
    let pixels = pixels(width, height, shade);
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET,
        CPUAccessFlags: 0,
        MiscFlags: D3D11_RESOURCE_MISC_SHARED,
    };
    let data = D3D11_SUBRESOURCE_DATA {
        pSysMem: pixels.as_ptr() as _,
        SysMemPitch: width * 4,
        SysMemSlicePitch: 0,
    };
    let mut raw = ptr::null_mut();
    assert_eq!((*device).CreateTexture2D(&desc, &data, &mut raw), 0);
    Com(raw)
}

fn pixels(width: u32, height: u32, shade: u8) -> Vec<u8> {
    [shade, shade, shade, 255].repeat((width * height) as usize)
}

fn check_picture(
    decoder: &mut Decoder,
    frame: VideoFrame,
    width: u32,
    height: u32,
    bright: bool,
    pts: i64,
) {
    let frames = match frame.union.unwrap() {
        video_frame::Union::H264s(frames) | video_frame::Union::H265s(frames) => frames,
        _ => panic!("unexpected codec"),
    };
    let mut decoded = 0;
    for packet in frames.frames {
        assert_eq!(packet.pts, pts);
        for picture in decoder.decode(&packet.data).unwrap() {
            assert_eq!((picture.width, picture.height), (width as _, height as _));
            let center = (height as usize / 2) * picture.linesize[0] as usize + width as usize / 2;
            let y = picture.data[0][center];
            assert!(
                if bright { y > 160 } else { y < 90 },
                "stale or overwritten pixels: expected bright={}, got Y={}",
                bright,
                y
            );
            decoded += 1;
        }
    }
    assert!(decoded > 0, "encoder output did not decode to a picture");
}

#[test]
#[ignore = "requires hardware H.264/H.265 encoders; run explicitly on GPU hosts"]
fn hardware_repeats_own_pixels_preserve_watchdog_and_reset_on_recreation() {
    unsafe {
        let mut raw_factory = ptr::null_mut();
        assert_eq!(
            CreateDXGIFactory1(&IDXGIFactory1::uuidof(), &mut raw_factory),
            0
        );
        let factory = Com(raw_factory as *mut IDXGIFactory1);
        let mut tested = 0;
        for index in 0.. {
            let mut raw_adapter = ptr::null_mut();
            let result = (*factory.0).EnumAdapters1(index, &mut raw_adapter);
            if result == DXGI_ERROR_NOT_FOUND {
                break;
            }
            assert_eq!(result, 0);
            let adapter = Com(raw_adapter);
            let mut desc = mem::zeroed();
            assert_eq!((*adapter.0).GetDesc1(&mut desc), 0);
            let vendor = match desc.VendorId {
                0x10de => Driver::NV,
                0x1002 => Driver::AMF,
                0x8086 => Driver::MFX,
                _ => continue,
            };
            let luid = ((desc.AdapterLuid.HighPart as i64) << 32) | desc.AdapterLuid.LowPart as i64;
            let mut raw_device = ptr::null_mut();
            let mut raw_context = ptr::null_mut();
            assert_eq!(
                D3D11CreateDevice(
                    adapter.0 as _,
                    D3D_DRIVER_TYPE_UNKNOWN,
                    ptr::null_mut(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                    ptr::null(),
                    0,
                    D3D11_SDK_VERSION,
                    &mut raw_device,
                    ptr::null_mut(),
                    &mut raw_context,
                ),
                0
            );
            let device = Com(raw_device);
            let context = Com(raw_context);
            for driver in [Driver::FFMPEG, vendor.clone()] {
                for format in [DataFormat::H264, DataFormat::H265] {
                    for (width, height) in [(320, 240), (640, 360)] {
                        let ctx = EncodeContext {
                            f: FeatureContext {
                                driver: driver.clone(),
                                vendor: vendor.clone(),
                                luid,
                                data_format: format,
                            },
                            d: DynamicContext {
                                device: Some(device.0 as _),
                                width: width as _,
                                height: height as _,
                                kbitrate: 1000,
                                framerate: 30,
                                gop: MAX_GOP as _,
                            },
                        };
                        let native = match Encoder::new(ctx.clone()) {
                            Ok(encoder) => encoder,
                            Err(()) => {
                                eprintln!(
                                    "SKIP unavailable {:?} {:?} {:?} adapter={luid:#x}",
                                    vendor, driver, format
                                );
                                continue;
                            }
                        };
                        let mut encoder = VRamEncoder {
                            encoder: native,
                            format,
                            ctx,
                            bitrate: 1000,
                            last_frame_len: 0,
                            same_bad_len_counter: 0,
                        };
                        assert!(encoder.encode_to_message(EncodeInput::Repeat, 0).is_err());
                        let mut decoder = Decoder::new(DecodeContext {
                            name: if format == DataFormat::H264 {
                                "h264"
                            } else {
                                "hevc"
                            }
                            .into(),
                            device_type: AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
                            thread_count: 1,
                        })
                        .unwrap();
                        let source = texture(device.0, width, height, 32);
                        let frame = encoder
                            .encode_to_message(EncodeInput::Texture((source.0 as _, 0)), 0)
                            .unwrap();
                        check_picture(&mut decoder, frame, width, height, false, 0);

                        // Overwrite and release the capture resource before any repeats.
                        let overwritten = pixels(width, height, 224);
                        (*context.0).UpdateSubresource(
                            source.0 as _,
                            0,
                            ptr::null(),
                            overwritten.as_ptr() as _,
                            width * 4,
                            0,
                        );
                        (*context.0).Flush();
                        drop(source);
                        encoder.same_bad_len_counter = 7;
                        encoder.last_frame_len = 42;
                        for count in 1..=100 {
                            let pts = count * 100;
                            let frame =
                                encoder.encode_to_message(EncodeInput::Repeat, pts).unwrap();
                            check_picture(&mut decoder, frame, width, height, false, pts);
                            assert_eq!(encoder.same_bad_len_counter, 7);
                            assert_eq!(encoder.last_frame_len, 42);
                        }

                        let source = texture(device.0, width, height, 224);
                        let frame = encoder
                            .encode_to_message(EncodeInput::Texture((source.0 as _, 0)), 10100)
                            .unwrap();
                        check_picture(&mut decoder, frame, width, height, true, 10100);
                        drop(source);
                        encoder.encoder.set_bitrate(1500).unwrap();
                        let frame = encoder
                            .encode_to_message(EncodeInput::Repeat, 10200)
                            .unwrap();
                        check_picture(&mut decoder, frame, width, height, true, 10200);
                        eprintln!(
                            "PASS {:?} {:?} {:?} {width}x{height} adapter={luid:#x}",
                            vendor, driver, format
                        );
                        tested += 1;
                    }
                }
            }
        }
        assert!(tested > 0, "no hardware encoders were tested");
        eprintln!("Validated {tested} encoder/codec/resolution combinations");
    }
}
