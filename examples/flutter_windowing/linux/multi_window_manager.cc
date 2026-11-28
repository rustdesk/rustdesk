//
// Created by yangbin on 2022/1/11.
//

#include "multi_window_manager.h"

#include <iostream>
#define RLOCK_WINDOW pthread_rwlock_rdlock(this->get_map_lock())
#define WLOCK_WINDOW pthread_rwlock_wrlock(this->get_map_lock())
#define UNLOCK_WINDOW pthread_rwlock_unlock(this->get_map_lock())

namespace {
int64_t g_next_id_ = 0;

class FlutterMainWindow : public BaseFlutterWindow {

 public:

  FlutterMainWindow(GtkWidget *window, std::unique_ptr<WindowChannel> window_channel)
      : window_channel_(std::move(window_channel)), window_(window) {}

  WindowChannel *GetWindowChannel() override {
    return window_channel_.get();
  }
 protected:
  GtkWindow *GetWindow() override {
    return GTK_WINDOW(window_);
  }

 private:
  std::unique_ptr<WindowChannel> window_channel_;
  GtkWidget *window_;

};

}

// static
MultiWindowManager *MultiWindowManager::Instance() {
  static auto manager = std::make_shared<MultiWindowManager>();
  return manager.get();
}

MultiWindowManager::MultiWindowManager() : windows_() {
  this->windows_map_lock_ = PTHREAD_RWLOCK_INITIALIZER;
}

MultiWindowManager::~MultiWindowManager() = default;

int64_t MultiWindowManager::Create(const std::string &args) {
  g_next_id_++;
  int64_t id = g_next_id_;
  auto window = std::make_unique<FlutterWindow>(id, args, shared_from_this());
  window->GetWindowChannel()->SetMethodHandler([this](int64_t from_window_id,
                                                      int64_t target_window_id,
                                                      const gchar *method,
                                                      FlValue *arguments,
                                                      FlMethodCall *method_call) {
    HandleMethodCall(from_window_id, target_window_id, method, arguments, method_call);
  });
  WLOCK_WINDOW;
  windows_[id] = std::move(window);
  UNLOCK_WINDOW;
  return id;
}

void MultiWindowManager::AttachMainWindow(GtkWidget *main_flutter_window,
                                          std::unique_ptr<WindowChannel> window_channel) {
  RLOCK_WINDOW;
  if (windows_.count(0) != 0) {
    g_critical("AttachMainWindow : main window already exists.");
    UNLOCK_WINDOW;
    return;
  }
  UNLOCK_WINDOW;
  window_channel->SetMethodHandler([this](int64_t from_window_id,
                                          int64_t target_window_id,
                                          const gchar *method,
                                          FlValue *arguments,
                                          FlMethodCall *method_call) {
    HandleMethodCall(from_window_id, target_window_id, method, arguments, method_call);
  });
  WLOCK_WINDOW;
  windows_[0] = std::make_unique<FlutterMainWindow>(main_flutter_window, std::move(window_channel));
  UNLOCK_WINDOW;
}

void MultiWindowManager::HandleMethodCall(int64_t from_window_id,
                                          int64_t target_window_id,
                                          const gchar *method,
                                          FlValue *arguments,
                                          FlMethodCall *method_call
) {
  RLOCK_WINDOW;
  if (windows_.count(target_window_id) == 0) {
    fl_method_call_respond_error(method_call, "-1", "target window not found.", nullptr, nullptr);
    UNLOCK_WINDOW;
    return;
  }
  UNLOCK_WINDOW;

  RLOCK_WINDOW;
  auto window_channel = windows_[target_window_id]->GetWindowChannel();
  if (!window_channel) {
    fl_method_call_respond_error(method_call, "-1", "target window channel not found.", nullptr, nullptr);
    UNLOCK_WINDOW;
    return;
  }
  window_channel->InvokeMethod(from_window_id, method, arguments, method_call);
  UNLOCK_WINDOW;
}

void MultiWindowManager::Show(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Show();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::Focus(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Focus();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::Hide(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Hide();
  }
  UNLOCK_WINDOW;
}

bool MultiWindowManager::IsHidden(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    bool ret = window->second->IsHidden();
    UNLOCK_WINDOW;
    return ret;
  }
  UNLOCK_WINDOW;
  return false;
}

void MultiWindowManager::Close(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Close();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::SetFullscreen(int64_t id, bool fullscreen) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->SetFullscreen(fullscreen);
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::SetFrame(int64_t id, double x, double y, double width, double height) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->SetBounds(x, y, width, height);
  }
  UNLOCK_WINDOW;
}

FlValue* MultiWindowManager::GetFrame(int64_t id) {
  FlValue* frame = NULL;
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    frame = window->second->GetBounds();
  } else {
    frame = fl_value_new_map();
  }
  UNLOCK_WINDOW;
  return frame;
}

void MultiWindowManager::SetTitle(int64_t id, const std::string &title) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->SetTitle(title);
  }
  UNLOCK_WINDOW;
}

std::vector<int64_t> MultiWindowManager::GetAllSubWindowIds() {
  RLOCK_WINDOW;
  std::vector<int64_t> ids;
  for (auto &window : windows_) {
    if (window.first != 0) {
      ids.push_back(window.first);
    }
  }
  UNLOCK_WINDOW;
  return ids;
}

void MultiWindowManager::Center(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Center();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::StartDragging(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->StartDragging();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::Minimize(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Minimize();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::Maximize(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Maximize();
  }
  UNLOCK_WINDOW;
}

bool MultiWindowManager::IsMaximized(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    UNLOCK_WINDOW;
    return window->second->IsMaximized();
  }
  UNLOCK_WINDOW;
  return false;
}

bool MultiWindowManager::IsMinimized(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    UNLOCK_WINDOW;
    return window->second->IsMinimized();
  }
  UNLOCK_WINDOW;
  return false;
}

void MultiWindowManager::Unmaximize(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->Unmaximize();
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::ShowTitlebar(int64_t id, bool show) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->ShowTitlebar(show);
  }
  UNLOCK_WINDOW;
}

void MultiWindowManager::OnWindowClose(int64_t id) {}

void MultiWindowManager::OnWindowDestroy(int64_t id) {
  std::cout << "destory id " << id << std::endl;
  WLOCK_WINDOW;
  windows_.erase(id);
  UNLOCK_WINDOW;
}

void MultiWindowManager::StartResizing(int64_t id, FlValue *value) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->StartResizing(value);
  }
  UNLOCK_WINDOW;
}

bool MultiWindowManager::IsPreventClose(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    bool ret = window->second->IsPreventClose();
    UNLOCK_WINDOW;
    return ret;
  }
  UNLOCK_WINDOW;
  return false;
}

void MultiWindowManager::SetPreventClose(int64_t id, bool setPreventClose) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    window->second->SetPreventClose(setPreventClose);
  }
  UNLOCK_WINDOW;
}

pthread_rwlock_t* MultiWindowManager::get_map_lock() {
  return &this->windows_map_lock_;
}

int64_t MultiWindowManager::GetXID(int64_t id) {
  RLOCK_WINDOW;
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    auto xid = window->second->GetXID();
    UNLOCK_WINDOW;
    return xid;
  }
  UNLOCK_WINDOW;
  return -1;
}

bool MultiWindowManager::IsFullScreen(int64_t id) {
  auto window = windows_.find(id);
  if (window != windows_.end()) {
    return window->second->IsFullScreen();
  }
  return false;
}
