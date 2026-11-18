#pragma once

#include <windows.h>
#include <gdiplus.h>
#include <algorithm>
#include <memory>
#include <string>

inline void PaintPrivacyWallpaper(HDC hdc, const RECT& bounds, Gdiplus::Bitmap& image,
	int style, bool tile)
{
	Gdiplus::Graphics graphics(hdc);
	graphics.SetInterpolationMode(Gdiplus::InterpolationModeHighQualityBicubic);
	if (tile)
	{
		Gdiplus::TextureBrush brush(&image, Gdiplus::WrapModeTile);
		brush.TranslateTransform(static_cast<float>(bounds.left), static_cast<float>(bounds.top));
		graphics.FillRectangle(&brush, Gdiplus::Rect(
			static_cast<int>(bounds.left), static_cast<int>(bounds.top),
			static_cast<int>(bounds.right - bounds.left), static_cast<int>(bounds.bottom - bounds.top)));
		return;
	}
	const float width = static_cast<float>(bounds.right - bounds.left);
	const float height = static_cast<float>(bounds.bottom - bounds.top);
	float imageWidth = static_cast<float>(image.GetWidth());
	float imageHeight = static_cast<float>(image.GetHeight());
	if (style == 2)
	{
		imageWidth = width;
		imageHeight = height;
	}
	else if (style != 0)
	{
		const float scale = style == 6
			? (std::min)(width / imageWidth, height / imageHeight)
			: (std::max)(width / imageWidth, height / imageHeight);
		imageWidth *= scale;
		imageHeight *= scale;
	}
	graphics.DrawImage(&image, Gdiplus::RectF(
		bounds.left + (width - imageWidth) / 2, bounds.top + (height - imageHeight) / 2,
		imageWidth, imageHeight));
}

// Loaded by the existing privacy window thread, after GDI+ initialization.
// The broker is launched in the active user's session, so it can resolve the
// selected desktop wallpaper without copying it through the controller.
class PrivacyBackground
{
public:
	void Reload(const std::wstring& directory)
	{
		image_.reset();
		custom_ = true;
		taskbar_ = false;
		wallpaper_style_ = 10;
		tile_wallpaper_ = false;
		wchar_t environment[32] = {};
		GetEnvironmentVariableW(L"RUSTDESK_PRIVACY_BACKGROUND", environment, ARRAYSIZE(environment));
		if (_wcsicmp(environment, L"wallpaper") == 0)
		{
			taskbar_ = true;
			wchar_t style[16] = {};
			DWORD bytes = sizeof(style);
			if (RegGetValueW(HKEY_CURRENT_USER, L"Control Panel\\Desktop", L"WallpaperStyle",
				RRF_RT_REG_SZ, nullptr, style, &bytes) == ERROR_SUCCESS)
			{
				wallpaper_style_ = _wtoi(style);
			}
			bytes = sizeof(style);
			if (RegGetValueW(HKEY_CURRENT_USER, L"Control Panel\\Desktop", L"TileWallpaper",
				RRF_RT_REG_SZ, nullptr, style, &bytes) == ERROR_SUCCESS)
			{
				tile_wallpaper_ = _wtoi(style) == 1;
			}
			wchar_t wallpaper[MAX_PATH] = {};
			if (SystemParametersInfoW(SPI_GETDESKWALLPAPER, ARRAYSIZE(wallpaper), wallpaper, 0) != FALSE)
			{
				if (TryLoadImage(wallpaper))
				{
					return;
				}
			}

			// SPI_GETDESKWALLPAPER can be unavailable while the process is on the
			// secure lock desktop. The per-user registry value remains available
			// and lets us retry after WTS_SESSION_UNLOCK.
			HKEY key = nullptr;
			if (RegOpenKeyExW(
				HKEY_CURRENT_USER,
				L"Control Panel\\Desktop",
				0,
				KEY_READ,
				&key) == ERROR_SUCCESS)
			{
				DWORD type = 0;
				DWORD size = sizeof(wallpaper);
				if (RegQueryValueExW(
					key,
					L"WallPaper",
					nullptr,
					&type,
					reinterpret_cast<LPBYTE>(wallpaper),
					&size) == ERROR_SUCCESS
					&& (type == REG_SZ || type == REG_EXPAND_SZ)
					&& wallpaper[0] != L'\0'
				)
				{
					TryLoadImage(wallpaper);
				}
				RegCloseKey(key);
			}
			return;
		}
		if (_wcsicmp(environment, L"black") == 0)
		{
			return;
		}

		wchar_t mode[32] = {};
		const auto config = directory + L"\\privacy-background.ini";
		GetPrivateProfileStringW(L"Privacy", L"Background", L"black", mode,
			ARRAYSIZE(mode), config.c_str());
		custom_ = _wcsicmp(mode, L"notice") != 0;
		if (_wcsicmp(mode, L"wallpaper") == 0)
		{
			taskbar_ = true;
			TryLoadImage(directory + L"\\privacy-wallpaper.png");
		}
	}

	bool ShowsTaskbar() const { return taskbar_; }

	// Return false to leave the original privacy notice painting path untouched.
	bool Paint(HDC hdc, const RECT& client, HWND window = nullptr) const
	{
		if (!custom_)
		{
			return false;
		}
		FillRect(hdc, &client, image_ ? GetSysColorBrush(COLOR_DESKTOP)
			: static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
		if (image_)
		{
			RECT bounds = client;
			if (window && (wallpaper_style_ == 22 || tile_wallpaper_))
			{
				bounds.left = GetSystemMetrics(SM_XVIRTUALSCREEN);
				bounds.top = GetSystemMetrics(SM_YVIRTUALSCREEN);
				bounds.right = bounds.left + GetSystemMetrics(SM_CXVIRTUALSCREEN);
				bounds.bottom = bounds.top + GetSystemMetrics(SM_CYVIRTUALSCREEN);
				MapWindowPoints(nullptr, window, reinterpret_cast<POINT*>(&bounds), 2);
			}
			PaintPrivacyWallpaper(hdc, bounds, *image_, wallpaper_style_, tile_wallpaper_);
		}
		return true;
	}

private:
	bool TryLoadImage(const std::wstring& path)
	{
		auto image = std::make_unique<Gdiplus::Bitmap>(path.c_str());
		if (image->GetLastStatus() == Gdiplus::Ok && image->GetWidth() && image->GetHeight())
		{
			image_ = std::move(image);
			return true;
		}
		else
		{
			OutputDebugStringW(L"RustDesk: privacy wallpaper unavailable; using black.\n");
		}
		return false;
	}

	bool custom_ = false;
	bool taskbar_ = false;
	int wallpaper_style_ = 10;
	bool tile_wallpaper_ = false;
	std::unique_ptr<Gdiplus::Bitmap> image_;
};
