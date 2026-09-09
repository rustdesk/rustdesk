use super::{wrap_hresult, ComPtr};
use crate::EncodeInput;
use std::{io, mem, ptr};
use winapi::um::d3d11::*;

#[derive(Default)]
pub struct RepeatTexture {
    texture: Option<ComPtr<ID3D11Texture2D>>,
    rotation: usize,
}

impl RepeatTexture {
    pub fn update(&mut self, frame: &EncodeInput) -> io::Result<()> {
        let EncodeInput::Texture((source, rotation)) = frame else {
            self.texture = None;
            return Ok(());
        };
        let source = *source as *mut ID3D11Texture2D;
        if source.is_null() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        unsafe {
            let mut device = ptr::null_mut();
            (*source).GetDevice(&mut device);
            let device = ComPtr(device);
            if device.is_null() {
                return Err(io::ErrorKind::NotConnected.into());
            }
            let mut context = ptr::null_mut();
            (*device.0).GetImmediateContext(&mut context);
            let context = ComPtr(context);
            if context.is_null() {
                return Err(io::ErrorKind::NotConnected.into());
            }
            if self.texture.is_none() {
                let mut desc = mem::zeroed();
                (*source).GetDesc(&mut desc);
                desc.Usage = D3D11_USAGE_DEFAULT;
                desc.CPUAccessFlags = 0;
                desc.BindFlags = D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET;
                desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED;
                let mut texture = ptr::null_mut();
                let result = (*device.0).CreateTexture2D(&desc, ptr::null(), &mut texture);
                let texture = ComPtr(texture);
                wrap_hresult(result)?;
                self.texture = Some(texture);
            }
            if let Some(texture) = &self.texture {
                // ReleaseFrame invalidates the DXGI surface even if we retain its COM pointer.
                (*context.0).CopyResource(texture.0 as *mut _, source as *mut _);
                (*context.0).Flush();
            }
        }
        self.rotation = *rotation;
        Ok(())
    }

    pub fn frame(&self) -> Option<EncodeInput<'_>> {
        self.texture
            .as_ref()
            .map(|texture| EncodeInput::RepeatTexture((texture.0 as *mut _, self.rotation)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::{
        shared::{dxgiformat::DXGI_FORMAT_B8G8R8A8_UNORM, dxgitype::DXGI_SAMPLE_DESC},
        um::d3dcommon::D3D_DRIVER_TYPE_WARP,
    };

    #[test]
    fn repeat_texture_owns_pixels_and_reuses_allocation() {
        unsafe {
            let mut device = ptr::null_mut();
            let mut context = ptr::null_mut();
            wrap_hresult(D3D11CreateDevice(
                ptr::null_mut(),
                D3D_DRIVER_TYPE_WARP,
                ptr::null_mut(),
                0,
                ptr::null(),
                0,
                D3D11_SDK_VERSION,
                &mut device,
                ptr::null_mut(),
                &mut context,
            ))
            .unwrap();
            let device = ComPtr(device);
            let context = ComPtr(context);
            let mut desc = D3D11_TEXTURE2D_DESC {
                Width: 4,
                Height: 4,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut repeat = RepeatTexture::default();
            assert!(repeat.frame().is_none());
            let mut saved_pointer: *mut std::ffi::c_void = ptr::null_mut();
            for color in [0xFF112233u32, 0xFF445566u32] {
                let pixels = [color; 16];
                let data = D3D11_SUBRESOURCE_DATA {
                    pSysMem: pixels.as_ptr() as *const _,
                    SysMemPitch: 16,
                    SysMemSlicePitch: 64,
                };
                let mut source = ptr::null_mut();
                wrap_hresult((*device.0).CreateTexture2D(&desc, &data, &mut source)).unwrap();
                let source = ComPtr(source);
                repeat
                    .update(&EncodeInput::Texture((source.0 as *mut _, 90)))
                    .unwrap();
                let frame = repeat.frame().unwrap();
                assert!(matches!(&frame, EncodeInput::RepeatTexture(_)));
                let (texture, rotation) = frame.texture().unwrap();
                assert_eq!(rotation, 90);
                assert_ne!(texture, source.0 as *mut _);
                if !saved_pointer.is_null() {
                    assert_eq!(texture, saved_pointer);
                }
                saved_pointer = texture;
                // Changing and releasing the source must not change the saved desktop.
                let overwrite = [0u32; 16];
                (*context.0).UpdateSubresource(
                    source.0 as *mut _,
                    0,
                    ptr::null(),
                    overwrite.as_ptr() as *const _,
                    16,
                    64,
                );
                drop(source);
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
                let mut staging = ptr::null_mut();
                wrap_hresult((*device.0).CreateTexture2D(&desc, ptr::null(), &mut staging))
                    .unwrap();
                let staging = ComPtr(staging);
                (*context.0).CopyResource(staging.0 as *mut _, texture as *mut _);
                let mut mapped = mem::zeroed();
                wrap_hresult((*context.0).Map(
                    staging.0 as *mut _,
                    0,
                    D3D11_MAP_READ,
                    0,
                    &mut mapped,
                ))
                .unwrap();
                for row in 0..4 {
                    let pixels = std::slice::from_raw_parts(
                        (mapped.pData as *const u8).add(row * mapped.RowPitch as usize)
                            as *const u32,
                        4,
                    );
                    assert_eq!(pixels, &[color; 4]);
                }
                (*context.0).Unmap(staging.0 as *mut _, 0);
                desc.Usage = D3D11_USAGE_DEFAULT;
                desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
                desc.CPUAccessFlags = 0;
            }
            repeat.update(&EncodeInput::YUV(&[])).unwrap();
            assert!(repeat.frame().is_none());
        }
    }
}
