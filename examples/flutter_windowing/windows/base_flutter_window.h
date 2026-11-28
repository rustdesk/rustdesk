//
// Created by yangbin on 2022/1/27.
//

#ifndef MULTI_WINDOW_WINDOWS_BASE_FLUTTER_WINDOW_H_
#define MULTI_WINDOW_WINDOWS_BASE_FLUTTER_WINDOW_H_

#include "window_channel.h"

class BaseFlutterWindow {

 public:

  virtual ~BaseFlutterWindow() = default;

  virtual WindowChannel *GetWindowChannel() = 0;

  void Show();

  void Hide();

  bool IsHidden();

  void Close();

  void SetTitle(const std::string &title);

  void Focus();

  bool IsFullscreen();

  void SetFullscreen(bool fullscreen);

  void Maximize(const flutter::EncodableMap& args);

  void SetTitleBarStyle(const flutter::EncodableMap& args);

  void SetAsFrameless();

  void Restore();

  void SetBounds(double_t x, double_t y, double_t width, double_t height);
  flutter::EncodableMap GetBounds();

  void Center();

  void StartDragging();

  void Minimize();

  bool IsMinimized();

  bool IsMaximized();

  void Maximize();

  void Unmaximize();

  void ShowTitlebar(bool show);

  void SetInitBackgroundColor(const flutter::EncodableMap *args);

  void StartResizing(const flutter::EncodableMap *param);

  bool IsPreventClose();

  void SetPreventClose(bool setPreventClose);

  bool IsFrameless();

  void ForceChildRefresh();

  std::string title_bar_style_ = "normal";

  virtual HWND GetWindowHandle() = 0;

  inline bool IsEraseTransparent() {
    return erase_transparent_;
  }

  inline COLORREF GetEraseBackgroundColor() {
    return erase_background_color_;
  }

private:
	bool g_is_window_fullscreen = false;
	std::string g_title_bar_style_before_fullscreen;
	bool g_is_frameless_before_fullscreen;
	RECT g_frame_before_fullscreen;
	bool g_maximized_before_fullscreen;
	LONG g_style_before_fullscreen;
	LONG g_ex_style_before_fullscreen;
	bool is_frameless_ = false;
  bool is_prevent_close_ = false;

  COLORREF erase_background_color_ = RGB(255, 255, 255);
  bool erase_transparent_ = false;

  bool is_first_move_ = true;
};

#endif //MULTI_WINDOW_WINDOWS_BASE_FLUTTER_WINDOW_H_
