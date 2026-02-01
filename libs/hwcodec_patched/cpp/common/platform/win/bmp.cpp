#include <atomic>
#include <chrono>
#include <cstdio>
#include <list>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include <d3d11.h>
#include <dxgi.h>
#include <wrl/client.h>

#include "../../common.h"
#include "win.h"

#define IF_FAILED_THROW(X)                                                     \
  if (FAILED(hr = (X))) {                                                      \
    throw hr;                                                                  \
  }

using Microsoft::WRL::ComPtr;

static HRESULT CreateBmpFile(LPCWSTR wszBmpFile, BYTE *pData,
                             const UINT uiFrameSize, const UINT uiWidth,
                             const UINT uiHeight) {
  HRESULT hr = S_OK;

  HANDLE hFile = INVALID_HANDLE_VALUE;
  DWORD dwWritten;
  UINT uiStride;

  BYTE header24[54] = {0x42, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                       0x00, 0x36, 0x00, 0x00, 0x00, 0x28, 0x00, 0x00, 0x00,
                       0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
                       0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                       0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                       0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00};

  DWORD dwSizeFile = uiWidth * uiHeight * 3;
  dwSizeFile += 54;
  header24[2] = dwSizeFile & 0x000000ff;
  header24[3] = static_cast<BYTE>((dwSizeFile & 0x0000ff00) >> 8);
  header24[4] = static_cast<BYTE>((dwSizeFile & 0x00ff0000) >> 16);
  header24[5] = (dwSizeFile & 0xff000000) >> 24;
  dwSizeFile -= 54;
  header24[18] = uiWidth & 0x000000ff;
  header24[19] = (uiWidth & 0x0000ff00) >> 8;
  header24[20] = static_cast<BYTE>((uiWidth & 0x00ff0000) >> 16);
  header24[21] = (uiWidth & 0xff000000) >> 24;

  header24[22] = uiHeight & 0x000000ff;
  header24[23] = (uiHeight & 0x0000ff00) >> 8;
  header24[24] = static_cast<BYTE>((uiHeight & 0x00ff0000) >> 16);
  header24[25] = (uiHeight & 0xff000000) >> 24;

  header24[34] = dwSizeFile & 0x000000ff;
  header24[35] = (dwSizeFile & 0x0000ff00) >> 8;
  header24[36] = static_cast<BYTE>((dwSizeFile & 0x00ff0000) >> 16);
  header24[37] = static_cast<BYTE>((dwSizeFile & 0xff000000) >> 24);

  try {
    hFile = CreateFileW(wszBmpFile, GENERIC_WRITE, FILE_SHARE_READ, NULL,
                        CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);

    IF_FAILED_THROW(hFile == INVALID_HANDLE_VALUE ? E_FAIL : S_OK);

    IF_FAILED_THROW(WriteFile(hFile, (LPCVOID)header24, 54, &dwWritten, 0) ==
                    FALSE);
    IF_FAILED_THROW(dwWritten == 0 ? E_FAIL : S_OK);

    uiStride = uiWidth * 3;
    BYTE *Tmpbufsrc = pData + (uiFrameSize - uiStride);

    for (UINT i = 0; i < uiHeight; i++) {

      IF_FAILED_THROW(WriteFile(hFile, (LPCVOID)Tmpbufsrc, uiStride, &dwWritten,
                                0) == FALSE);
      IF_FAILED_THROW(dwWritten == 0 ? E_FAIL : S_OK);

      Tmpbufsrc -= uiStride;
    }
  } catch (HRESULT) {
  }

  if (hFile != INVALID_HANDLE_VALUE)
    CloseHandle(hFile);

  return hr;
}

static std::string GetDirectoryFromFilename(const std::string &filename) {
  size_t lastSeparator = filename.find_last_of("/\\");

  if (lastSeparator != std::string::npos) {
    return filename.substr(0, lastSeparator);
  }

  return "";
}

static bool createBgraBmpFile(ID3D11Device *device, ID3D11Texture2D *texture,
                              const std::string &filename) {
  D3D11_TEXTURE2D_DESC desc = {};
  ComPtr<ID3D11DeviceContext> deviceContext;
  HRESULT hr;
  texture->GetDesc(&desc);
  desc.Usage = D3D11_USAGE_STAGING;
  desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
  desc.BindFlags = 0;
  ComPtr<ID3D11Texture2D> bgraStagingTexture;
  hr = device->CreateTexture2D(&desc, nullptr,
                               bgraStagingTexture.GetAddressOf());
  IF_FAILED_THROW(hr);
  device->GetImmediateContext(deviceContext.ReleaseAndGetAddressOf());
  deviceContext->CopyResource(bgraStagingTexture.Get(), texture);

  D3D11_MAPPED_SUBRESOURCE ResourceDesc = {};
  deviceContext->Map(bgraStagingTexture.Get(), 0, D3D11_MAP_READ, 0,
                     &ResourceDesc);

  UINT uiImageSize = desc.Width * desc.Height * 3;
  BYTE *pDataRgb = new (std::nothrow) BYTE[uiImageSize];
  BYTE *pDataRgbaColor = (BYTE *)ResourceDesc.pData;
  BYTE *pDataRgbColor = pDataRgb;
  for (UINT i = 0; i < desc.Height; i++) {
    for (UINT j = 0; j < desc.Width; j++) {
      if (desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM) {
        // bgr             bgra
        *pDataRgbColor++ = *pDataRgbaColor++;
        *pDataRgbColor++ = *pDataRgbaColor++;
        *pDataRgbColor++ = *pDataRgbaColor++;
        pDataRgbaColor++;
      } else {
        // bgr             rgba
        pDataRgbColor[0] = pDataRgbaColor[2];
        pDataRgbColor[1] = pDataRgbaColor[1];
        pDataRgbColor[2] = pDataRgbaColor[0];
        pDataRgbColor += 3;
        pDataRgbaColor += 4;
      }
    }
  }

  auto dir = GetDirectoryFromFilename(filename);
  DWORD attrib = GetFileAttributesA(dir.c_str());
  if (attrib == INVALID_FILE_ATTRIBUTES ||
      !(attrib & FILE_ATTRIBUTE_DIRECTORY)) {
    if (!CreateDirectoryA(dir.c_str(), NULL)) {
      std::cout << "Failed to create directory: " << dir << std::endl;
      return false;
    } else {
      std::cout << "Directory created: " << dir << std::endl;
    }
  } else {
    // already exists
  }
  int size = MultiByteToWideChar(CP_UTF8, 0, filename.c_str(), -1, nullptr, 0);
  wchar_t *wszBmpFile = new wchar_t[size];
  MultiByteToWideChar(CP_UTF8, 0, filename.c_str(), -1, wszBmpFile, size);

  hr =
      CreateBmpFile(wszBmpFile, pDataRgb, uiImageSize, desc.Width, desc.Height);
  delete[] pDataRgb;
  delete[] wszBmpFile;
  IF_FAILED_THROW(hr);
  deviceContext->Unmap(bgraStagingTexture.Get(), 0);
}

void SaveBgraBmps(ID3D11Device *device, void *texture, int cycle) {
  if (!texture)
    return;
  static int index = 0;
  if (index++ % cycle == 0) {
    auto now = std::chrono::system_clock::now();
    auto time_t_now = std::chrono::system_clock::to_time_t(now);

    std::tm local_tm;
    localtime_s(&local_tm, &time_t_now);

    char buffer[80];
    std::strftime(buffer, 80, "%H_%M_%S", &local_tm);
    std::string filename = std::string("bmps") + "/" + std::to_string(index) +
                           "_" + buffer + ".bmp";
    createBgraBmpFile(device, (ID3D11Texture2D *)texture, filename);
  }
}