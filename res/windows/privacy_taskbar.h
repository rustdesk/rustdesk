#pragma once

#include <windows.h>
#include <dwmapi.h>
#include <shlobj.h>
#include <shellapi.h>
#include <algorithm>
#include <string>
#include <vector>

#pragma comment(lib, "dwmapi.lib")
#pragma comment(lib, "shell32.lib")
#pragma comment(lib, "ole32.lib")

// Only the notification area is live. Application icons come from pinned
// shortcuts, so running applications and their activity cannot enter the image.
class PrivacyTaskbar
{
public:
	PrivacyTaskbar() { LoadPinnedIcons(); }
	PrivacyTaskbar(const PrivacyTaskbar&) = delete;
	PrivacyTaskbar& operator=(const PrivacyTaskbar&) = delete;
	~PrivacyTaskbar()
	{
		Reset();
		for (auto icon : icons_) DestroyIcon(icon);
	}

	bool Update(HWND destination)
	{
		Search search = { MonitorFromWindow(destination, MONITOR_DEFAULTTONULL), nullptr };
		EnumWindows(FindTaskbar, reinterpret_cast<LPARAM>(&search));
		RECT screen = {};
		if (!search.window || !IsWindowVisible(search.window) || !GetWindowRect(search.window, &screen))
		{
			if (!IsRectEmpty(&bounds_)) InvalidateRect(destination, &bounds_, FALSE);
			Reset();
			return false;
		}
		RECT bounds = screen;
		MapWindowPoints(nullptr, destination, reinterpret_cast<POINT*>(&bounds), 2);
		if (!EqualRect(&bounds_, &bounds))
		{
			InvalidateRect(destination, &bounds_, FALSE);
			bounds_ = bounds;
			InvalidateRect(destination, &bounds_, FALSE);
		}
		const auto oldDpi = dpi_;
		const auto oldTray = tray_;
		const auto getDpiForWindow = reinterpret_cast<UINT(WINAPI*)(HWND)>(
			GetProcAddress(GetModuleHandleW(L"user32.dll"), "GetDpiForWindow"));
		dpi_ = getDpiForWindow ? getDpiForWindow(search.window) : 0;
		if (!dpi_)
		{
			const auto dc = GetDC(search.window);
			if (dc)
			{
				dpi_ = GetDeviceCaps(dc, LOGPIXELSX);
				ReleaseDC(search.window, dc);
			}
		}
		if (!dpi_) dpi_ = 96;
		tray_ = {};
		const auto trayWindow = FindWindowExW(search.window, nullptr, L"TrayNotifyWnd", nullptr);
		if (trayWindow && GetWindowRect(trayWindow, &tray_))
			MapWindowPoints(nullptr, destination, reinterpret_cast<POINT*>(&tray_), 2);
		if (oldDpi != dpi_ || !EqualRect(&oldTray, &tray_))
			InvalidateRect(destination, &bounds_, FALSE);
		if (IsRectEmpty(&tray_))
		{
			ResetThumbnail();
			return true;
		}
		if (source_ != search.window || !thumbnail_)
		{
			ResetThumbnail();
			if (FAILED(DwmRegisterThumbnail(destination, search.window, &thumbnail_)))
			{
				OutputDebugStringW(L"RustDesk: cannot register privacy system tray thumbnail.\n");
				return true;
			}
			source_ = search.window;
		}
		DWM_THUMBNAIL_PROPERTIES properties = {};
		properties.dwFlags = DWM_TNP_RECTDESTINATION | DWM_TNP_RECTSOURCE
			| DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY;
		properties.rcDestination = tray_;
		properties.rcSource = tray_;
		OffsetRect(&properties.rcSource, -bounds_.left, -bounds_.top);
		properties.fVisible = TRUE;
		properties.opacity = 255;
		properties.fSourceClientAreaOnly = FALSE;
		if (FAILED(DwmUpdateThumbnailProperties(thumbnail_, &properties)))
		{
			OutputDebugStringW(L"RustDesk: cannot update privacy system tray thumbnail.\n");
			ResetThumbnail();
		}
		return true;
	}

	void Paint(HDC dc) const
	{
		if (IsRectEmpty(&bounds_)) return;
		DWORD light = 1, bytes = sizeof(light);
		RegGetValueW(HKEY_CURRENT_USER,
			L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
			L"SystemUsesLightTheme", RRF_RT_REG_DWORD, nullptr, &light, &bytes);
		const auto brush = CreateSolidBrush(light ? RGB(243, 243, 243) : RGB(32, 32, 32));
		FillRect(dc, &bounds_, brush);
		DeleteObject(brush);

		const bool horizontal = bounds_.right - bounds_.left >= bounds_.bottom - bounds_.top;
		const int slot = MulDiv(44, dpi_, 96);
		const int iconSize = MulDiv(24, dpi_, 96);
		const int padding = MulDiv(12, dpi_, 96);
		int start = (horizontal ? bounds_.left : bounds_.top) + padding;
		const int end = horizontal
			? (IsRectEmpty(&tray_) ? bounds_.right : tray_.left)
			: (IsRectEmpty(&tray_) ? bounds_.bottom : tray_.top);
		DWORD centered = 0;
		bytes = sizeof(centered);
		RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced",
			L"TaskbarAl", RRF_RT_REG_DWORD, nullptr, &centered, &bytes);
		if (centered && horizontal)
		{
			const int group = slot * static_cast<int>(icons_.size() + 1);
			start = (std::max)(start, (std::min)(static_cast<int>((bounds_.left + bounds_.right - group) / 2), end - group));
		}
		const int cross = horizontal
			? (bounds_.top + bounds_.bottom - iconSize) / 2
			: (bounds_.left + bounds_.right - iconSize) / 2;
		const auto logo = CreateSolidBrush(RGB(0, 120, 215));
		const int square = (iconSize - MulDiv(2, dpi_, 96)) / 2;
		for (int y = 0; y < 2; ++y)
			for (int x = 0; x < 2; ++x)
			{
				RECT rect = { (horizontal ? start + (slot - iconSize) / 2 : cross) + x * (iconSize - square),
					(horizontal ? cross : start + (slot - iconSize) / 2) + y * (iconSize - square), 0, 0 };
				rect.right = rect.left + square;
				rect.bottom = rect.top + square;
				FillRect(dc, &rect, logo);
			}
		DeleteObject(logo);
		start += slot;
		for (auto icon : icons_)
		{
			if (start + slot > end) break;
			DrawIconEx(dc, horizontal ? start + (slot - iconSize) / 2 : cross,
				horizontal ? cross : start + (slot - iconSize) / 2,
				icon, iconSize, iconSize, 0, nullptr, DI_NORMAL);
			start += slot;
		}
	}

private:
	static HICON LoadPinnedIcon(const std::wstring& path)
	{
		IShellLinkW* link = nullptr;
		if (FAILED(CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&link))))
			return nullptr;
		IPersistFile* file = nullptr;
		HICON icon = nullptr;
		if (SUCCEEDED(link->QueryInterface(IID_PPV_ARGS(&file))))
		{
			if (SUCCEEDED(file->Load(path.c_str(), STGM_READ)))
			{
				wchar_t location[MAX_PATH] = {}, expanded[MAX_PATH] = {};
				int index = 0;
				if (SUCCEEDED(link->GetIconLocation(location, ARRAYSIZE(location), &index)) && location[0])
				{
					ExpandEnvironmentStringsW(location, expanded, ARRAYSIZE(expanded));
					ExtractIconExW(expanded, index, &icon, nullptr, 1);
				}
				if (!icon)
				{
					PIDLIST_ABSOLUTE target = nullptr;
					if (SUCCEEDED(link->GetIDList(&target)) && target)
					{
						SHFILEINFOW info = {};
						if (SHGetFileInfoW(reinterpret_cast<LPCWSTR>(target), 0, &info, sizeof(info),
							SHGFI_PIDL | SHGFI_ICON | SHGFI_LARGEICON)) icon = info.hIcon;
						CoTaskMemFree(target);
					}
				}
			}
			file->Release();
		}
		link->Release();
		return icon;
	}

	void LoadPinnedIcons()
	{
		const auto initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
		PWSTR roaming = nullptr;
		if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_RoamingAppData, 0, nullptr, &roaming)))
		{
			const auto directory = std::wstring(roaming)
				+ L"\\Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar\\";
			CoTaskMemFree(roaming);
			WIN32_FIND_DATAW data = {};
			const auto find = FindFirstFileW((directory + L"*.lnk").c_str(), &data);
			if (find != INVALID_HANDLE_VALUE)
			{
				do
				{
					const auto icon = LoadPinnedIcon(directory + data.cFileName);
					if (icon) icons_.push_back(icon);
				} while (FindNextFileW(find, &data));
				FindClose(find);
			}
		}
		if (SUCCEEDED(initialized)) CoUninitialize();
	}

	struct Search { HMONITOR monitor; HWND window; };
	static BOOL CALLBACK FindTaskbar(HWND window, LPARAM parameter)
	{
		wchar_t name[64] = {};
		GetClassNameW(window, name, ARRAYSIZE(name));
		if (wcscmp(name, L"Shell_TrayWnd") != 0
			&& wcscmp(name, L"Shell_SecondaryTrayWnd") != 0) return TRUE;
		auto& search = *reinterpret_cast<Search*>(parameter);
		if (MonitorFromWindow(window, MONITOR_DEFAULTTONULL) != search.monitor) return TRUE;
		search.window = window;
		return FALSE;
	}

	void ResetThumbnail()
	{
		if (thumbnail_) DwmUnregisterThumbnail(thumbnail_);
		thumbnail_ = nullptr;
		source_ = nullptr;
	}
	void Reset()
	{
		ResetThumbnail();
		bounds_ = {};
		tray_ = {};
	}

	HWND source_ = nullptr;
	HTHUMBNAIL thumbnail_ = nullptr;
	RECT bounds_ = {};
	RECT tray_ = {};
	UINT dpi_ = 96;
	std::vector<HICON> icons_;
};
