# A real-time hardware codec library for [RustDesk](https://github.com/rustdesk/rustdesk) based on FFmpeg


## Codec

### Windows

| GPU           | FFmpeg ram        | FFmpeg vram | sdk vram |
| ------------- | ----------------  | ----------- | -------- |
| intel encode  | qsv               | qsv         | Y        |
| intel decode  | d3d11             | d3d11       | Y        |
| nvidia encode | nvenc(nv12->d3d11)| nvenc(d3d11)| Y        |
| nvidia decode | d3d11             | d3d11       | N        |
| amd encode    | amf               | amf         | Y        |
| amd decode    | d3d11             | d3d11       | Y        |

#### Notes

* The reason for discarding the codecs using Cucontext is discussed in the following forum thread: https://forums.developer.nvidia.com/t/cuctxdestroy-causing-system-freeze-and-black-screen/290542/1.
Based on the information above, there are several optimizations and changes made to the codec:
  - FFmpeg encoding AV_PIX_FMT_NV12 directly: The codec is modified to transfer AV_PIX_FMT_NV12 to AV_PIX_FMT_D3D11. This is done because FFmpeg doesn't use Cucontext if the device type is AV_HWDEVICE_TYPE_D3D11VA.
  - FFmpeg decoding with AV_HWDEVICE_TYPE_CUDA acceleration: This functionality is disabled and replaced with AV_HWDEVICE_TYPE_D3D11VA. The decoding process now utilizes D3D11VA acceleration instead of CUDA.
  - SDK decoding with CUDA acceleration: The CUDA acceleration support is disabled.

* amd sdk remove h265 support, https://github.com/GPUOpen-LibrariesAndSDKs/AMF/issues/432

### Linux

| GPU           | FFmpeg ram     |
| ------------- | -------------- |
| intel encode  | vaapi          |
| intel decode  | vaapi          |
| nvidia encode | vaapi, nvnec   |
| nvidia decode | vaapi, nvdec   |
| amd encode    | vaapi, amf     |
| amd decode    | vaapi          |

#### Issue

* vaapi: only tested on intel with `va-driver-all`, and hevc_vaapi encoding not supported on my pc
* remove hevc_vaapi because of possible poor quality
* amf: not tested, https://github.com/GPUOpen-LibrariesAndSDKs/AMF/issues/378

### MacOS

| FFmpeg ram encode   | FFmpeg ram decode   |
| ------------------  | ------------------  |
| h265 only           | Y                   |

### Android

| FFmpeg ram encode   |
| ------------------  |
| Y                   |

## System requirements

* intel

  Windows Intel(r) graphics driver since 27.20.100.8935 version. 

  [Hardware Platforms Supported by the Intel(R) Media SDK GPU Runtime](https://www.intel.com/content/www/us/en/docs/onevpl/upgrade-from-msdk/2023-1/onevpl-hardware-support-details.html#HARDWARE-PLATFORMS-SUPPORTED-BY-THE-INTEL-R-MEDIA-SDK-GPU-RUNTIME)

  https://www.intel.com/content/www/us/en/docs/onevpl/developer-reference-media-intel-hardware/1-1/overview.html

* AMD

  AMD Radeon Software Adrenalin Edition 23.1.2 (22.40.01.34) or newer

  https://github.com/GPUOpen-LibrariesAndSDKs/AMF

* nvidia

  Windows: Driver version 471.41 or higher

  https://docs.nvidia.com/video-technologies/video-codec-sdk/11.1/read-me/index.html

  https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new?ncid=em-prod-816193

