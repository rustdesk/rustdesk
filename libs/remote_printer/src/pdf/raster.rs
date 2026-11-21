use hbb_common::{
    anyhow::Context,
    bail,
    libloading::os::windows::{Library, LOAD_LIBRARY_SEARCH_SYSTEM32},
    ResultType,
};
use std::ffi::c_void;
use windows::{
    core::{IUnknown, Interface, HRESULT},
    Data::Pdf::PdfPage,
    Win32::{
        Foundation::{HMODULE, POINT},
        Graphics::{
            Direct2D::Common::{D2D_COLOR_F, D2D_RECT_F},
            Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::{
                D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
                D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
                D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                IDXGIDevice, IDXGISurface,
            },
        },
        System::WinRT::Pdf::{IPdfRendererNative, PDF_RENDER_PARAMS},
    },
};

const BYTES_PER_PIXEL: usize = 4;
const FIRST_SUBRESOURCE: u32 = 0;
const PDF_FACTORY: &[u8] = b"PdfCreateRenderer\0";
type CreateRenderer = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;

pub struct Rasterizer {
    renderer: IPdfRendererNative,
    context: ID3D11DeviceContext,
    device: ID3D11Device,
    // The renderer must be released before its DLL is unloaded.
    _pdf_library: Library,
}

impl Rasterizer {
    pub fn new() -> ResultType<Self> {
        let pdf_library = load_pdf_library()?;
        let mut device = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let device = device.context("D3D11CreateDevice returned no device")?;
        let context = context.context("D3D11CreateDevice returned no context")?;
        let renderer = create_renderer(&pdf_library, &device.cast::<IDXGIDevice>()?)?;
        Ok(Self {
            renderer,
            context,
            device,
            _pdf_library: pdf_library,
        })
    }

    pub fn render(&self, page: &PdfPage, extent: [u32; 2]) -> ResultType<Vec<u8>> {
        let size = page.Size()?;
        let description = texture_description(extent);
        let texture = self.texture(&description)?;
        let parameters = PDF_RENDER_PARAMS {
            SourceRect: D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: size.Width,
                bottom: size.Height,
            },
            DestinationWidth: extent[0],
            DestinationHeight: extent[1],
            BackgroundColor: D2D_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            IgnoreHighContrast: true,
        };
        unsafe {
            self.renderer.RenderPageToSurface(
                &page.cast::<IUnknown>()?,
                &texture.cast::<IDXGISurface>()?,
                POINT::default(),
                Some(&parameters),
            )?;
        }
        self.read_pixels(&texture, description)
    }

    fn texture(&self, description: &D3D11_TEXTURE2D_DESC) -> ResultType<ID3D11Texture2D> {
        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(description, None, Some(&mut texture))?
        };
        texture.context("CreateTexture2D returned no texture")
    }

    fn read_pixels(
        &self,
        source: &ID3D11Texture2D,
        description: D3D11_TEXTURE2D_DESC,
    ) -> ResultType<Vec<u8>> {
        let staging = self.texture(&D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..description
        })?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context.CopyResource(&staging, source);
            self.context.Map(
                &staging,
                FIRST_SUBRESOURCE,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;
        }
        let result = copy_rows(&mapped, [description.Width, description.Height]);
        unsafe { self.context.Unmap(&staging, FIRST_SUBRESOURCE) };
        result
    }
}

pub(super) fn load_pdf_library() -> ResultType<Library> {
    // An optional backend must not add a process startup DLL dependency.
    let library =
        unsafe { Library::load_with_flags("windows.data.pdf.dll", LOAD_LIBRARY_SEARCH_SYSTEM32)? };
    unsafe { library.get::<CreateRenderer>(PDF_FACTORY)? };
    Ok(library)
}

fn create_renderer(library: &Library, device: &IDXGIDevice) -> ResultType<IPdfRendererNative> {
    let create = unsafe { library.get::<CreateRenderer>(PDF_FACTORY)? };
    let mut renderer = std::ptr::null_mut();
    unsafe { create(device.as_raw(), &mut renderer).ok()? };
    if renderer.is_null() {
        bail!("The Windows PDF factory returned no renderer");
    }
    Ok(unsafe { IPdfRendererNative::from_raw(renderer) })
}

fn texture_description(extent: [u32; 2]) -> D3D11_TEXTURE2D_DESC {
    D3D11_TEXTURE2D_DESC {
        Width: extent[0],
        Height: extent[1],
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
        ..Default::default()
    }
}

fn copy_rows(mapped: &D3D11_MAPPED_SUBRESOURCE, extent: [u32; 2]) -> ResultType<Vec<u8>> {
    let row_bytes = (extent[0] as usize)
        .checked_mul(BYTES_PER_PIXEL)
        .context("Row size overflow")?;
    let length = row_bytes
        .checked_mul(extent[1] as usize)
        .context("Image size overflow")?;
    if mapped.pData.is_null() || (mapped.RowPitch as usize) < row_bytes {
        bail!("The mapped PDF image has an invalid row layout");
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(length)?;
    for row in 0..extent[1] as usize {
        let offset = row
            .checked_mul(mapped.RowPitch as usize)
            .context("Row offset overflow")?;
        let bytes =
            unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(offset), row_bytes) };
        pixels.extend_from_slice(bytes);
    }
    Ok(pixels)
}
