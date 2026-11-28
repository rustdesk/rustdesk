//
// Created by yangbin on 2022/1/11.
//

#ifndef DESKTOP_MULTI_WINDOW_WINDOWS_MULTI_WINDOW_MANAGER_H_
#define DESKTOP_MULTI_WINDOW_WINDOWS_MULTI_WINDOW_MANAGER_H_

#include <cstdint>
#include <string>
#include <map>
#include <cmath>
#include <vector>
#include <pthread.h>

#include "base_flutter_window.h"
#include "flutter_window.h"

class MultiWindowManager : public std::enable_shared_from_this<MultiWindowManager>, public FlutterWindowCallback {

 public:
  static MultiWindowManager *Instance();

  MultiWindowManager();

  virtual ~MultiWindowManager();

  int64_t Create(const std::string &args);

  void AttachMainWindow(GtkWidget *main_flutter_window, std::unique_ptr<WindowChannel> window_channel);

  void Show(int64_t id);

  void Hide(int64_t id);

  bool IsHidden(int64_t id);

  void Focus(int64_t id);

  void Close(int64_t id);

  bool IsFullScreen(int64_t id);

  void SetFullscreen(int64_t id, bool fullscreen);

  void SetFrame(int64_t id, double_t x, double_t y, double_t width, double_t height);
  FlValue* GetFrame(int64_t id);

  void Center(int64_t id);

  void SetTitle(int64_t id, const std::string &title);

  std::vector<int64_t> GetAllSubWindowIds();

  void OnWindowClose(int64_t id) override;

  void OnWindowDestroy(int64_t id) override;

  void StartDragging(int64_t id);

  void Minimize(int64_t id);

  void Maximize(int64_t id);

  bool IsMaximized(int64_t id);

  bool IsMinimized(int64_t id);

  void Unmaximize(int64_t id);

  void ShowTitlebar(int64_t id, bool show);

  void StartResizing(int64_t id, FlValue *value);

  bool IsPreventClose(int64_t id);

  void SetPreventClose(int64_t id, bool setPreventClose);

  int64_t GetXID(int64_t id);

private:
  std::map<int64_t, std::unique_ptr<BaseFlutterWindow>> windows_;
  pthread_rwlock_t windows_map_lock_;

  void HandleMethodCall(int64_t from_window_id,
                        int64_t target_window_id,
                        const gchar *method,
                        FlValue *arguments,
                        FlMethodCall *method_call
  );

  pthread_rwlock_t* get_map_lock();
};

#endif //DESKTOP_MULTI_WINDOW_WINDOWS_MULTI_WINDOW_MANAGER_H_
