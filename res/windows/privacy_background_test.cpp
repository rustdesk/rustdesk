#include "privacy_background.h"
#include <cstdio>
#include <stdexcept>

void Check(bool value, const char* message)
{
	if (!value) throw std::runtime_error(message);
}

int wmain()
{
	Gdiplus::GdiplusStartupInput input;
	ULONG_PTR token = 0;
	if (Gdiplus::GdiplusStartup(&token, &input, nullptr) != Gdiplus::Ok) return 1;
	wchar_t temporary[MAX_PATH] = {};
	wchar_t filename[MAX_PATH] = {};
	GetTempPathW(ARRAYSIZE(temporary), temporary);
	GetTempFileNameW(temporary, L"rdb", 0, filename);
	DeleteFileW(filename);
	const std::wstring directory = std::wstring(filename) + L"-\u58c1\u7eb8";
	CreateDirectoryW(directory.c_str(), nullptr);
	const auto config = directory + L"\\privacy-background.ini";
	const auto wallpaper = directory + L"\\privacy-wallpaper.png";
	HDC dc = CreateCompatibleDC(nullptr);
	BITMAPINFO info = {};
	info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
	info.bmiHeader.biWidth = 80;
	info.bmiHeader.biHeight = -60;
	info.bmiHeader.biPlanes = 1;
	info.bmiHeader.biBitCount = 32;
	void* bits = nullptr;
	HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
	HGDIOBJ previous = SelectObject(dc, bitmap);
	RECT client = { 0, 0, 80, 60 };
	int result = 0;
	try
	{
		Check(dc && bitmap, "create offscreen surface");
		_wputenv_s(L"RUSTDESK_PRIVACY_BACKGROUND", L"");
		PrivacyBackground background;
		background.Reload(directory);
		Check(background.Paint(dc, client), "missing config defaults to black");
		Check(GetPixel(dc, 40, 30) == RGB(0, 0, 0), "default black center pixel");
		std::puts("PASS default black background");
		_wputenv_s(L"RUSTDESK_PRIVACY_BACKGROUND", L"black");
		background.Reload(directory);
		Check(background.Paint(dc, client), "environment black selects custom paint");
		Check(GetPixel(dc, 40, 30) == RGB(0, 0, 0), "environment black center pixel");
		_wputenv_s(L"RUSTDESK_PRIVACY_BACKGROUND", L"");
		WritePrivateProfileStringW(L"Privacy", L"Background", L"unknown", config.c_str());
		background.Reload(directory);
		Check(background.Paint(dc, client), "unknown mode keeps opaque overlay");

		WritePrivateProfileStringW(L"Privacy", L"Background", L"black", config.c_str());
		background.Reload(directory);
		Check(background.Paint(dc, client), "black selects custom paint");
		Check(GetPixel(dc, 40, 30) == RGB(0, 0, 0), "black center pixel");
		Check(GetPixel(dc, 0, 0) == RGB(0, 0, 0), "black corner pixel");
		std::puts("PASS black background");
		{
			Gdiplus::Bitmap image(20, 40, static_cast<Gdiplus::PixelFormat>(0x26200A));
			{ Gdiplus::Graphics graphics(&image); graphics.Clear(Gdiplus::Color::Red); }
			FillRect(dc, &client, static_cast<HBRUSH>(GetStockObject(BLACK_BRUSH)));
			PaintPrivacyWallpaper(dc, client, image, 6, false);
			Check(GetPixel(dc, 40, 30) == RGB(255, 0, 0), "fit keeps image center");
			Check(GetPixel(dc, 5, 30) == RGB(0, 0, 0), "fit preserves letterbox instead of cropping");
			PaintPrivacyWallpaper(dc, client, image, 10, false);
			Check(GetPixel(dc, 5, 30) == RGB(255, 0, 0), "fill covers the letterbox");
			std::puts("PASS Windows fit and fill produce different visible layouts");
		}

		{
			Gdiplus::Bitmap image(20, 40, static_cast<Gdiplus::PixelFormat>(0x26200A));
			Gdiplus::Graphics graphics(&image);
			graphics.Clear(Gdiplus::Color::Red);
			const CLSID png = { 0x557cf406, 0x1a04, 0x11d3, {0x9a, 0x73, 0, 0, 0xf8, 0x1e, 0xf3, 0x2e} };
			Check(image.Save(wallpaper.c_str(), &png, nullptr) == Gdiplus::Ok, "write wallpaper fixture");
		}
		WritePrivateProfileStringW(L"Privacy", L"Background", L"wallpaper", config.c_str());
		background.Reload(directory);
		Check(background.Paint(dc, client), "wallpaper selects custom paint");
		Check(GetPixel(dc, 40, 30) == RGB(255, 0, 0), "wallpaper center pixel");
		Check(GetPixel(dc, 5, 5) == RGB(255, 0, 0), "portrait wallpaper fills landscape screen");
		WritePrivateProfileStringW(L"Privacy", L"Background", L"black", config.c_str());
		background.Reload(directory);
		Check(DeleteFileW(wallpaper.c_str()) != FALSE, "reload releases old image file");
		WritePrivateProfileStringW(L"Privacy", L"Background", L"wallpaper", config.c_str());
		background.Reload(directory);
		Check(background.Paint(dc, client), "missing wallpaper keeps opaque overlay");
		Check(GetPixel(dc, 40, 30) == RGB(0, 0, 0), "missing wallpaper falls back to black");
		std::puts("PASS wallpaper, reload and missing-image fallback");
	}
	catch (const std::exception& error)
	{
		std::fprintf(stderr, "FAIL %s\n", error.what());
		result = 1;
	}
	SelectObject(dc, previous);
	DeleteObject(bitmap);
	DeleteDC(dc);
	DeleteFileW(wallpaper.c_str());
	DeleteFileW(config.c_str());
	RemoveDirectoryW(directory.c_str());
	Gdiplus::GdiplusShutdown(token);
	return result;
}
