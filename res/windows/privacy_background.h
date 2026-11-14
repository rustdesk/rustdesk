#pragma once

#include <windows.h>
#include <gdiplus.h>
#include <algorithm>
#include <memory>
#include <string>

// Loaded by the existing privacy window thread, after GDI+ initialization.
// The broker can run as SYSTEM, so wallpaper is exported by the local user's
// configuration tool instead of reading that account's desktop settings here.
class PrivacyBackground
{
public:
	void Reload(const std::wstring& directory)
	{
		image_.reset();
		wchar_t mode[32] = {};
		const auto config = directory + L"\\privacy-background.ini";
		GetPrivateProfileStringW(L"Privacy", L"Background", L"notice", mode,
			ARRAYSIZE(mode), config.c_str());
		custom_ = _wcsicmp(mode, L"black") == 0 || _wcsicmp(mode, L"wallpaper") == 0;
		if (_wcsicmp(mode, L"wallpaper") == 0)
		{
			const auto path = directory + L"\\privacy-wallpaper.png";
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
	bool custom_ = false;
	std::unique_ptr<Gdiplus::Bitmap> image_;
};
