#pragma once

#include <windows.h>
#include <gdiplus.h>
#include <algorithm>
#include <memory>
#include <string>

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
		wchar_t environment[32] = {};
		GetEnvironmentVariableW(L"RUSTDESK_PRIVACY_BACKGROUND", environment, ARRAYSIZE(environment));
		if (_wcsicmp(environment, L"wallpaper") == 0)
		{
			wchar_t wallpaper[MAX_PATH] = {};
			if (SystemParametersInfoW(SPI_GETDESKWALLPAPER, ARRAYSIZE(wallpaper), wallpaper, 0) != FALSE)
			{
				LoadImage(wallpaper);
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
			LoadImage(directory + L"\\privacy-wallpaper.png");
		}
	}

	// Return false to leave the original privacy notice painting path untouched.
	bool Paint(HDC hdc, const RECT& client) const
	{
		if (!custom_)
		{
			return false;
		}
		FillRect(hdc, &client, static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
		if (image_)
		{
			const float width = static_cast<float>(client.right - client.left);
			const float height = static_cast<float>(client.bottom - client.top);
			const float scale = (std::max)(width / image_->GetWidth(), height / image_->GetHeight());
			const float imageWidth = image_->GetWidth() * scale;
			const float imageHeight = image_->GetHeight() * scale;
			Gdiplus::Graphics graphics(hdc);
			graphics.SetInterpolationMode(Gdiplus::InterpolationModeHighQualityBicubic);
			graphics.DrawImage(image_.get(), Gdiplus::RectF(
				client.left + (width - imageWidth) / 2, client.top + (height - imageHeight) / 2,
				imageWidth, imageHeight));
		}
		return true;
	}

private:
	void LoadImage(const std::wstring& path)
	{
		auto image = std::make_unique<Gdiplus::Bitmap>(path.c_str());
		if (image->GetLastStatus() == Gdiplus::Ok && image->GetWidth() && image->GetHeight())
		{
			image_ = std::move(image);
		}
		else
		{
			OutputDebugStringW(L"RustDesk: privacy wallpaper unavailable; using black.\n");
		}
	}

	bool custom_ = false;
	std::unique_ptr<Gdiplus::Bitmap> image_;
};
