// RustDesk OHOS controller NAPI wrapper.
//
// The Rust side (librustdesk.so) is built separately and is NOT linked at
// build time: every symbol from the FFI contract
// (docs/harmonyos-migration/ohos-controller-ffi.md) is resolved with
// dlopen/dlsym on first use. When the library or a symbol is missing, NAPI
// calls throw a JS error instead of crashing.
//
// Event/clipboard callbacks are marshalled to the JS thread with
// napi_threadsafe_function. The RGBA frame callback never enters the JS
// thread: the pixels are blitted straight into the XComponent surface's
// NativeWindow buffer on the calling (Rust) thread.

#include "napi/native_api.h"
#include "native_window/external_window.h"
#include "native_buffer/native_buffer.h"
#include "hilog/log.h"

#include <dlfcn.h>
#include <unistd.h>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>

#undef LOG_DOMAIN
#undef LOG_TAG
#define LOG_DOMAIN 0x5253
#define LOG_TAG "RustDeskNapi"

// ---------------------------------------------------------------------------
// librustdesk.so C ABI (contract v M1). Keep typedefs in sync with the doc.
// ---------------------------------------------------------------------------

typedef void (*rd_event_cb_t)(const char* json);
typedef void (*rd_rgba_cb_t)(int32_t display, const uint8_t* data, int32_t len,
                             int32_t w, int32_t h, int32_t stride);
typedef void (*rd_clipboard_cb_t)(const char* text);

typedef int32_t (*fn_ohos_init_t)(const char* home_dir, const char* app_dir,
                                  const char* custom_client_config);
typedef void (*fn_ohos_set_callbacks_t)(rd_event_cb_t event_cb, rd_rgba_cb_t rgba_cb,
                                        rd_clipboard_cb_t clipboard_cb);
typedef void (*fn_ohos_free_string_t)(char* s);
typedef const char* (*fn_ohos_version_t)(void);

typedef char* (*fn_ohos_session_new_t)(const char* peer_id, int32_t session_kind,
                                       const char* password, int32_t force_relay);
typedef int32_t (*fn_ohos_session_start_t)(const char* uuid);
typedef int32_t (*fn_ohos_session_login_t)(const char* uuid, const char* password,
                                           int32_t remember);
typedef int32_t (*fn_ohos_session_send2fa_t)(const char* uuid, const char* code,
                                             int32_t trust_this_device);
typedef int32_t (*fn_ohos_session_reconnect_t)(const char* uuid, int32_t force_relay);
typedef int32_t (*fn_ohos_session_close_t)(const char* uuid);

typedef int32_t (*fn_ohos_send_mouse_t)(const char* uuid, int32_t mask, int32_t x,
                                        int32_t y, int32_t alt, int32_t ctrl,
                                        int32_t shift, int32_t command);
typedef int32_t (*fn_ohos_input_key_t)(const char* uuid, const char* name, int32_t down,
                                       int32_t press, int32_t alt, int32_t ctrl,
                                       int32_t shift, int32_t command);
typedef int32_t (*fn_ohos_input_string_t)(const char* uuid, const char* value);
typedef int32_t (*fn_ohos_session_refresh_t)(const char* uuid);
typedef int32_t (*fn_ohos_switch_display_t)(const char* uuid, int32_t display);
typedef int32_t (*fn_ohos_send_clipboard_t)(const char* uuid, const char* text);

typedef int32_t (*fn_ohos_send_files_t)(const char* uuid, int32_t act_id, const char* path,
                                        const char* to, int32_t file_num,
                                        int32_t include_hidden, int32_t is_remote);
typedef int32_t (*fn_ohos_set_confirm_override_file_t)(const char* uuid, int32_t act_id,
                                                       int32_t file_num, int32_t need_override,
                                                       int32_t remember, int32_t is_upload);
typedef int32_t (*fn_ohos_cancel_job_t)(const char* uuid, int32_t act_id);
typedef int32_t (*fn_ohos_remove_file_t)(const char* uuid, int32_t act_id, const char* path,
                                         int32_t file_num, int32_t is_remote);
typedef int32_t (*fn_ohos_read_dir_t)(const char* uuid, const char* path,
                                      int32_t include_hidden, int32_t is_remote);
typedef char* (*fn_ohos_get_option_t)(const char* key);
typedef int32_t (*fn_ohos_set_option_t)(const char* key, const char* value);
typedef int32_t (*fn_ohos_record_screen_t)(const char* uuid, int32_t start);
typedef int32_t (*fn_ohos_request_voice_call_t)(const char* uuid);
typedef int32_t (*fn_ohos_close_voice_call_t)(const char* uuid);
typedef int32_t (*fn_ohos_answer_voice_call_t)(const char* uuid, int32_t accept);
typedef int32_t (*fn_ohos_send_clipboard_image_png_t)(const char* uuid, const char* png_base64);

// Display options (quality / codec preference / view-only), contract v M2.
typedef char* (*fn_ohos_session_get_option_t)(const char* uuid, const char* key);
typedef int32_t (*fn_ohos_session_set_option_t)(const char* uuid, const char* key,
                                                const char* value);
typedef int32_t (*fn_ohos_save_image_quality_t)(const char* uuid, const char* value);
typedef int32_t (*fn_ohos_save_custom_image_quality_t)(const char* uuid, int32_t quality);
typedef int32_t (*fn_ohos_set_custom_fps_t)(const char* uuid, int32_t fps);
typedef int32_t (*fn_ohos_toggle_option_t)(const char* uuid, const char* name);
typedef int32_t (*fn_ohos_get_toggle_option_t)(const char* uuid, const char* name);
typedef int32_t (*fn_ohos_update_supported_decodings_t)(const char* uuid);
typedef char* (*fn_ohos_alternative_codecs_t)(const char* uuid);

// Controlled side (controlled peer), contract v M3.
typedef int32_t (*fn_ohos_server_start_t)(void);
typedef int32_t (*fn_ohos_server_start_service_t)(void);
typedef int32_t (*fn_ohos_server_stop_service_t)(void);
typedef char* (*fn_ohos_get_server_id_t)(void);
typedef char* (*fn_ohos_get_server_password_t)(void);
typedef int32_t (*fn_ohos_update_server_password_t)(void);
typedef int32_t (*fn_ohos_has_permanent_password_t)(void);
typedef int32_t (*fn_ohos_server_authorize_t)(int32_t id, int32_t accept);
typedef int32_t (*fn_ohos_server_close_connection_t)(int32_t id);
typedef int32_t (*fn_ohos_cm_handle_incoming_voice_call_t)(int32_t id, int32_t accept);
typedef int32_t (*fn_ohos_cm_close_voice_call_t)(int32_t id);
typedef int32_t (*fn_ohos_cm_request_voice_call_t)(int32_t id);
typedef int32_t (*fn_ohos_server_switch_permission_t)(int32_t id, const char* name,
                                                      int32_t enabled);
typedef int32_t (*fn_ohos_server_send_chat_t)(int32_t id, const char* text);
typedef int32_t (*fn_ohos_server_push_clipboard_t)(const char* text, const char* html);
typedef int32_t (*fn_ohos_capture_start_t)(int32_t width, int32_t height);
typedef int32_t (*fn_ohos_capture_stop_t)(void);
typedef int32_t (*fn_ohos_capture_mic_t)(int32_t enable);
typedef int32_t (*fn_ohos_capture_running_t)(void);

struct RustFns {
    fn_ohos_init_t init = nullptr;
    fn_ohos_set_callbacks_t set_callbacks = nullptr;
    fn_ohos_free_string_t free_string = nullptr;
    fn_ohos_version_t version = nullptr;
    fn_ohos_session_new_t session_new = nullptr;
    fn_ohos_session_start_t session_start = nullptr;
    fn_ohos_session_login_t session_login = nullptr;
    fn_ohos_session_send2fa_t session_send2fa = nullptr;
    fn_ohos_session_reconnect_t session_reconnect = nullptr;
    fn_ohos_session_close_t session_close = nullptr;
    fn_ohos_send_mouse_t send_mouse = nullptr;
    fn_ohos_input_key_t input_key = nullptr;
    fn_ohos_input_string_t input_string = nullptr;
    fn_ohos_session_refresh_t session_refresh = nullptr;
    fn_ohos_switch_display_t switch_display = nullptr;
    fn_ohos_send_clipboard_t send_clipboard = nullptr;
    fn_ohos_send_files_t send_files = nullptr;
    fn_ohos_set_confirm_override_file_t set_confirm_override_file = nullptr;
    fn_ohos_cancel_job_t cancel_job = nullptr;
    fn_ohos_remove_file_t remove_file = nullptr;
    fn_ohos_read_dir_t read_dir = nullptr;
    fn_ohos_get_option_t get_option = nullptr;
    fn_ohos_set_option_t set_option = nullptr;
    fn_ohos_record_screen_t record_screen = nullptr;
    fn_ohos_request_voice_call_t request_voice_call = nullptr;
    fn_ohos_close_voice_call_t close_voice_call = nullptr;
    fn_ohos_answer_voice_call_t answer_voice_call = nullptr;
    fn_ohos_send_clipboard_image_png_t send_clipboard_image_png = nullptr;
    fn_ohos_session_get_option_t session_get_option = nullptr;
    fn_ohos_session_set_option_t session_set_option = nullptr;
    fn_ohos_save_image_quality_t save_image_quality = nullptr;
    fn_ohos_save_custom_image_quality_t save_custom_image_quality = nullptr;
    fn_ohos_set_custom_fps_t set_custom_fps = nullptr;
    fn_ohos_toggle_option_t toggle_option = nullptr;
    fn_ohos_get_toggle_option_t get_toggle_option = nullptr;
    fn_ohos_update_supported_decodings_t update_supported_decodings = nullptr;
    fn_ohos_alternative_codecs_t alternative_codecs = nullptr;
};

static RustFns g_fns;
static std::mutex g_lib_mutex;
static void* g_lib = nullptr;
static std::string g_lib_error;

static bool EnsureLib() {
    std::lock_guard<std::mutex> lock(g_lib_mutex);
    if (g_lib != nullptr) {
        return true;
    }
    if (!g_lib_error.empty()) {
        // Previous attempt failed; report the cached reason without retry noise.
        return false;
    }
    dlerror();
    g_lib = dlopen("librustdesk.so", RTLD_NOW | RTLD_LOCAL);
    if (g_lib == nullptr) {
        const char* err = dlerror();
        g_lib_error = err != nullptr ? err : "dlopen failed";
        OH_LOG_Print(LOG_APP, LOG_ERROR, LOG_DOMAIN, LOG_TAG,
                     "dlopen librustdesk.so failed: %{public}s", g_lib_error.c_str());
        return false;
    }
    struct Entry {
        const char* name;
        void** slot;
    };
    const Entry table[] = {
        {"ohos_init", reinterpret_cast<void**>(&g_fns.init)},
        {"ohos_set_callbacks", reinterpret_cast<void**>(&g_fns.set_callbacks)},
        {"ohos_free_string", reinterpret_cast<void**>(&g_fns.free_string)},
        {"ohos_version", reinterpret_cast<void**>(&g_fns.version)},
        {"ohos_session_new", reinterpret_cast<void**>(&g_fns.session_new)},
        {"ohos_session_start", reinterpret_cast<void**>(&g_fns.session_start)},
        {"ohos_session_login", reinterpret_cast<void**>(&g_fns.session_login)},
        {"ohos_session_send2fa", reinterpret_cast<void**>(&g_fns.session_send2fa)},
        {"ohos_session_reconnect", reinterpret_cast<void**>(&g_fns.session_reconnect)},
        {"ohos_session_close", reinterpret_cast<void**>(&g_fns.session_close)},
        {"ohos_send_mouse", reinterpret_cast<void**>(&g_fns.send_mouse)},
        {"ohos_input_key", reinterpret_cast<void**>(&g_fns.input_key)},
        {"ohos_input_string", reinterpret_cast<void**>(&g_fns.input_string)},
        {"ohos_session_refresh", reinterpret_cast<void**>(&g_fns.session_refresh)},
        {"ohos_switch_display", reinterpret_cast<void**>(&g_fns.switch_display)},
        {"ohos_send_clipboard", reinterpret_cast<void**>(&g_fns.send_clipboard)},
        {"ohos_send_files", reinterpret_cast<void**>(&g_fns.send_files)},
        {"ohos_set_confirm_override_file", reinterpret_cast<void**>(&g_fns.set_confirm_override_file)},
        {"ohos_cancel_job", reinterpret_cast<void**>(&g_fns.cancel_job)},
        {"ohos_remove_file", reinterpret_cast<void**>(&g_fns.remove_file)},
        {"ohos_read_dir", reinterpret_cast<void**>(&g_fns.read_dir)},
        {"ohos_get_option", reinterpret_cast<void**>(&g_fns.get_option)},
        {"ohos_set_option", reinterpret_cast<void**>(&g_fns.set_option)},
        {"ohos_record_screen", reinterpret_cast<void**>(&g_fns.record_screen)},
        {"ohos_request_voice_call", reinterpret_cast<void**>(&g_fns.request_voice_call)},
        {"ohos_close_voice_call", reinterpret_cast<void**>(&g_fns.close_voice_call)},
        {"ohos_send_clipboard_image_png",
         reinterpret_cast<void**>(&g_fns.send_clipboard_image_png)},
        {"ohos_session_get_option", reinterpret_cast<void**>(&g_fns.session_get_option)},
        {"ohos_session_set_option", reinterpret_cast<void**>(&g_fns.session_set_option)},
        {"ohos_save_image_quality", reinterpret_cast<void**>(&g_fns.save_image_quality)},
        {"ohos_save_custom_image_quality",
         reinterpret_cast<void**>(&g_fns.save_custom_image_quality)},
        {"ohos_set_custom_fps", reinterpret_cast<void**>(&g_fns.set_custom_fps)},
        {"ohos_toggle_option", reinterpret_cast<void**>(&g_fns.toggle_option)},
        {"ohos_get_toggle_option", reinterpret_cast<void**>(&g_fns.get_toggle_option)},
        {"ohos_update_supported_decodings",
         reinterpret_cast<void**>(&g_fns.update_supported_decodings)},
        {"ohos_alternative_codecs", reinterpret_cast<void**>(&g_fns.alternative_codecs)},
    };
    bool ok = true;
    for (const Entry& e : table) {
        *e.slot = dlsym(g_lib, e.name);
        if (*e.slot == nullptr) {
            ok = false;
            OH_LOG_Print(LOG_APP, LOG_ERROR, LOG_DOMAIN, LOG_TAG, "dlsym missing: %{public}s",
                         e.name);
        }
    }
    if (!ok) {
        g_lib_error = "librustdesk.so is missing contract symbols";
    }
    return ok;
}

static std::string LibMissingReason(const char* symbol) {
    if (!g_lib_error.empty()) {
        return std::string("librustdesk.so not usable (") + g_lib_error + ")";
    }
    return std::string("symbol not found in librustdesk.so: ") + symbol;
}

// Throws a JS Error and returns nullptr when the library cannot be used.
static void* RequireFn(napi_env env, const char* symbol) {
    if (!EnsureLib()) {
        napi_throw_error(env, "ERR_RUSTDESK_LIB", LibMissingReason(symbol).c_str());
        return nullptr;
    }
    void* fn = dlsym(g_lib, symbol);
    if (fn == nullptr) {
        napi_throw_error(env, "ERR_RUSTDESK_LIB", LibMissingReason(symbol).c_str());
        return nullptr;
    }
    return fn;
}

// ---------------------------------------------------------------------------
// NAPI small helpers
// ---------------------------------------------------------------------------

static napi_value MakeInt32(napi_env env, int32_t v) {
    napi_value out = nullptr;
    if (napi_create_int32(env, v, &out) != napi_ok) {
        return nullptr;
    }
    return out;
}

static napi_value ThrowErr(napi_env env, const std::string& msg) {
    napi_throw_error(env, "ERR_RUSTDESK_FFI", msg.c_str());
    return nullptr;
}

static bool GetArgStr(napi_env env, napi_value v, std::string* out) {
    size_t len = 0;
    if (napi_get_value_string_utf8(env, v, nullptr, 0, &len) != napi_ok) {
        return false;
    }
    out->assign(len, '\0');
    if (len > 0) {
        if (napi_get_value_string_utf8(env, v, &(*out)[0], len + 1, &len) != napi_ok) {
            return false;
        }
        out->resize(len);
    }
    return true;
}

static bool GetArgBool(napi_env env, napi_value v, bool* out) {
    bool b = false;
    if (napi_get_value_bool(env, v, &b) != napi_ok) {
        return false;
    }
    *out = b;
    return true;
}

static bool GetArgInt32(napi_env env, napi_value v, int32_t* out) {
    int32_t i = 0;
    if (napi_get_value_int32(env, v, &i) != napi_ok) {
        return false;
    }
    *out = i;
    return true;
}

struct CallArgs {
    size_t argc = 0;
    napi_value argv[8] = {nullptr};
};

static bool GetCallArgs(napi_env env, napi_callback_info info, size_t expect, CallArgs* out) {
    size_t argc = 8;
    napi_value* argv = out->argv;
    if (napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr) != napi_ok) {
        return false;
    }
    if (argc < expect) {
        napi_throw_type_error(env, "ERR_RUSTDESK_ARG", "missing arguments");
        return false;
    }
    out->argc = argc;
    return true;
}

// ---------------------------------------------------------------------------
// Threadsafe function bridge for event / clipboard callbacks
// ---------------------------------------------------------------------------

struct JsStringData {
    std::string text;
};

static napi_threadsafe_function g_event_tsfn = nullptr;
static napi_threadsafe_function g_clip_tsfn = nullptr;

// Runs on any Rust thread. Copies the string and hands it to the JS loop
// without ever blocking the caller.
static void OnEventFromRust(const char* json) {
    napi_threadsafe_function tsfn = g_event_tsfn;
    if (tsfn == nullptr) {
        return;
    }
    JsStringData* data = new JsStringData{json != nullptr ? json : ""};
    napi_status st = napi_call_threadsafe_function(tsfn, data, napi_tsfn_nonblocking);
    if (st != napi_ok) {
        delete data;
    }
}

static void OnClipboardFromRust(const char* text) {
    napi_threadsafe_function tsfn = g_clip_tsfn;
    if (tsfn == nullptr) {
        return;
    }
    JsStringData* data = new JsStringData{text != nullptr ? text : ""};
    napi_status st = napi_call_threadsafe_function(tsfn, data, napi_tsfn_nonblocking);
    if (st != napi_ok) {
        delete data;
    }
}

static void CallJsWithString(napi_env env, napi_value js_cb, void* context, void* data) {
    JsStringData* msg = static_cast<JsStringData*>(data);
    if (msg == nullptr) {
        return;
    }
    if (env != nullptr && js_cb != nullptr) {
        napi_value arg = nullptr;
        if (napi_create_string_utf8(env, msg->text.c_str(), msg->text.size(), &arg) == napi_ok) {
            napi_value undef = nullptr;
            napi_get_undefined(env, &undef);
            napi_call_function(env, undef, js_cb, 1, &arg, nullptr);
        }
    }
    delete msg;
}

// ---------------------------------------------------------------------------
// RGBA frame -> NativeWindow blit (render hot path, never touches JS)
// ---------------------------------------------------------------------------

static std::mutex g_win_mutex;
static OHNativeWindow* g_window = nullptr;

static void BlitFrame(int32_t display, const uint8_t* data, int32_t len, int32_t w, int32_t h,
                      int32_t stride) {
    (void)display;  // M1: single display.
    if (data == nullptr || len <= 0 || w <= 0 || h <= 0 || stride <= 0) {
        return;
    }
    static std::atomic<bool> g_nullwin_logged{false};
    std::lock_guard<std::mutex> lock(g_win_mutex);
    OHNativeWindow* win = g_window;
    if (win == nullptr) {
        if (!g_nullwin_logged.exchange(true)) {
            OH_LOG_Print(LOG_APP, LOG_ERROR, LOG_DOMAIN, LOG_TAG,
                         "BlitFrame called but native window is null");
        }
        return;
    }
    int32_t buf_w = 0;
    int32_t buf_h = 0;
    // GET_BUFFER_GEOMETRY takes [out] height first, then width. The surface can
    // be resized/recreated behind our back (layout change, screen off/on),
    // leaving the buffers at a transient size; re-assert the frame geometry
    // whenever the current buffer no longer matches.
    OH_NativeWindow_NativeWindowHandleOpt(win, GET_BUFFER_GEOMETRY, &buf_h, &buf_w);
    if (buf_w != w || buf_h != h) {
        OH_NativeWindow_NativeWindowHandleOpt(win, SET_BUFFER_GEOMETRY, w, h);
    }
    OHNativeWindowBuffer* nwb = nullptr;
    int fence = -1;
    if (OH_NativeWindow_NativeWindowRequestBuffer(win, &nwb, &fence) != 0 || nwb == nullptr) {
        if (fence >= 0) {
            close(fence);
        }
        return;
    }
    // The request fence marks when the consumer finished with this buffer; the
    // CPU writer must not hand it back to FlushBuffer (the compositor would
    // wait on the wrong object and the queue stalls on the first real fence).
    if (fence >= 0) {
        close(fence);
    }
    fence = -1;
    OH_NativeBuffer* nb = nullptr;
    void* vir = nullptr;
    if (OH_NativeBuffer_FromNativeWindowBuffer(nwb, &nb) != 0 || nb == nullptr ||
        OH_NativeBuffer_Map(nb, &vir) != 0 || vir == nullptr) {
        // Borrowed pointer: OH_NativeBuffer_FromNativeWindowBuffer does not take a
        // reference, so never OH_NativeBuffer_Unreference it here.
        if (nb != nullptr) {
            OH_NativeBuffer_Unmap(nb);
        }
        OH_NativeWindow_NativeWindowFlushBuffer(win, nwb, fence, Region{nullptr, 0});
        return;
    }
    // The consumer may still hand out buffers at its own size even after
    // SET_BUFFER_GEOMETRY, so read the truth from the allocation and
    // nearest-neighbor map the frame onto whatever we got.
    int32_t dst_w = 0;
    int32_t dst_h = 0;
    int32_t dst_stride = 0;
    BufferHandle* bh = OH_NativeWindow_GetBufferHandleFromNative(nwb);
    if (bh != nullptr) {
        dst_w = bh->width;
        dst_h = bh->height;
        dst_stride = bh->stride;
    }
    if (dst_w <= 0) {
        dst_w = buf_w > 0 ? buf_w : w;
    }
    if (dst_h <= 0) {
        dst_h = buf_h > 0 ? buf_h : h;
    }
    if (dst_stride <= 0) {
        dst_stride = dst_w * 4;
    }
    static std::atomic<bool> g_geom_logged{false};
    if (!g_geom_logged.exchange(true)) {
        int32_t fmt = 0;
        OH_NativeWindow_NativeWindowHandleOpt(win, GET_FORMAT, &fmt);
        OH_LOG_Print(LOG_APP, LOG_INFO, LOG_DOMAIN, LOG_TAG,
                     "BlitFrame geom: src %{public}dx%{public}d srcStride=%{public}d "
                     "len=%{public}d dst=%{public}dx%{public}d dstStride=%{public}d fmt=%{public}d",
                     w, h, stride, len, dst_w, dst_h, dst_stride, fmt);
    }
    const uint8_t* src = data;
    uint8_t* dst = static_cast<uint8_t*>(vir);
    // Row copies must stay in small chunks: a full-row memcpy (>= a few KB)
    // takes the libc non-temporal path whose stores the emulator compositor
    // never observes, leaving the surface black. 256-byte chunks do not.
    const int32_t kChunk = 256;
    if (dst_w == w && dst_h == h) {
        for (int32_t r = 0; r < dst_h; ++r) {
            const uint8_t* srow = src + static_cast<size_t>(r) * static_cast<size_t>(stride);
            uint8_t* drow = dst + static_cast<size_t>(r) * static_cast<size_t>(dst_stride);
            if (static_cast<size_t>(r) * static_cast<size_t>(stride) +
                    static_cast<size_t>(w) * 4 > static_cast<size_t>(len)) {
                break;
            }
            const int32_t rowBytes = w * 4;
            int32_t off = 0;
            for (; off + kChunk <= rowBytes; off += kChunk) {
                memcpy(drow + off, srow + off, kChunk);
            }
            if (off < rowBytes) {
                memcpy(drow + off, srow + off, rowBytes - off);
            }
        }
    } else {
        for (int32_t r = 0; r < dst_h; ++r) {
            int32_t sy = static_cast<int32_t>(static_cast<int64_t>(r) * h / dst_h);
            if (sy >= h) {
                sy = h - 1;
            }
            size_t src_off = static_cast<size_t>(sy) * static_cast<size_t>(stride);
            if (src_off + static_cast<size_t>(w) * 4 > static_cast<size_t>(len)) {
                break;
            }
            uint8_t* drow = dst + static_cast<size_t>(r) * static_cast<size_t>(dst_stride);
            const uint8_t* srow = src + src_off;
            for (int32_t cx = 0; cx < dst_w; ++cx) {
                int32_t sx = static_cast<int32_t>(static_cast<int64_t>(cx) * w / dst_w);
                memcpy(drow + static_cast<size_t>(cx) * 4, srow + static_cast<size_t>(sx) * 4, 4);
            }
        }
    }
    OH_NativeBuffer_Unmap(nb);
    Region region;
    region.rects = nullptr;
    region.rectNumber = 0;
    OH_NativeWindow_NativeWindowFlushBuffer(win, nwb, fence, region);
}

// ---------------------------------------------------------------------------
// Exported NAPI functions
// ---------------------------------------------------------------------------

static napi_value NapiInit(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 3, &args)) {
        return nullptr;
    }
    std::string home;
    std::string app_dir;
    std::string custom;
    if (!GetArgStr(env, args.argv[0], &home) || !GetArgStr(env, args.argv[1], &app_dir) ||
        !GetArgStr(env, args.argv[2], &custom)) {
        return ThrowErr(env, "init: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_init");
    if (fn == nullptr) {
        return nullptr;
    }
    int32_t rc = reinterpret_cast<fn_ohos_init_t>(fn)(home.c_str(), app_dir.c_str(),
                                                      custom.c_str());
    return MakeInt32(env, rc);
}

static napi_value NapiSetCallbacks(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    static std::atomic<bool> registered{false};
    if (registered.load()) {
        return MakeInt32(env, 0);
    }
    napi_value resource_name = nullptr;
    napi_create_string_utf8(env, "rustdeskCallbacks", NAPI_AUTO_LENGTH, &resource_name);
    if (napi_create_threadsafe_function(env, args.argv[0], nullptr, resource_name, 0, 1,
                                        nullptr, nullptr, nullptr, CallJsWithString,
                                        &g_event_tsfn) != napi_ok) {
        return ThrowErr(env, "setCallbacks: failed to create event threadsafe function");
    }
    napi_value clip_name = nullptr;
    napi_create_string_utf8(env, "rustdeskClipboard", NAPI_AUTO_LENGTH, &clip_name);
    if (napi_create_threadsafe_function(env, args.argv[1], nullptr, clip_name, 0, 1, nullptr,
                                        nullptr, nullptr, CallJsWithString,
                                        &g_clip_tsfn) != napi_ok) {
        napi_release_threadsafe_function(g_event_tsfn, napi_tsfn_release);
        g_event_tsfn = nullptr;
        return ThrowErr(env, "setCallbacks: failed to create clipboard threadsafe function");
    }
    void* fn = RequireFn(env, "ohos_set_callbacks");
    if (fn == nullptr) {
        napi_release_threadsafe_function(g_event_tsfn, napi_tsfn_release);
        napi_release_threadsafe_function(g_clip_tsfn, napi_tsfn_release);
        g_event_tsfn = nullptr;
        g_clip_tsfn = nullptr;
        return nullptr;
    }
    reinterpret_cast<fn_ohos_set_callbacks_t>(fn)(OnEventFromRust, BlitFrame,
                                                  OnClipboardFromRust);
    registered.store(true);
    return MakeInt32(env, 0);
}

static napi_value NapiInitVideo(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string surface_id;
    if (!GetArgStr(env, args.argv[0], &surface_id)) {
        return ThrowErr(env, "initVideo: surfaceId must be a string");
    }
    uint64_t sid = strtoull(surface_id.c_str(), nullptr, 10);
    OHNativeWindow* win = nullptr;
    int32_t rc = OH_NativeWindow_CreateNativeWindowFromSurfaceId(sid, &win);
    if (rc != 0 || win == nullptr) {
        return ThrowErr(env, "initVideo: CreateNativeWindowFromSurfaceId failed");
    }
    std::lock_guard<std::mutex> lock(g_win_mutex);
    if (g_window != nullptr) {
        OH_NativeWindow_DestroyNativeWindow(g_window);
    }
    g_window = win;
    return MakeInt32(env, 0);
}

static napi_value NapiSessionNew(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 4, &args)) {
        return nullptr;
    }
    std::string peer_id;
    std::string password;
    int32_t session_kind = 0;
    bool force_relay = false;
    if (!GetArgStr(env, args.argv[0], &peer_id) || !GetArgInt32(env, args.argv[1], &session_kind) ||
        !GetArgStr(env, args.argv[2], &password) || !GetArgBool(env, args.argv[3], &force_relay)) {
        return ThrowErr(env, "sessionNew: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_session_new");
    if (fn == nullptr) {
        return nullptr;
    }
    char* uuid = reinterpret_cast<fn_ohos_session_new_t>(fn)(
        peer_id.c_str(), session_kind, password.c_str(), force_relay ? 1 : 0);
    std::string result = uuid != nullptr ? uuid : "";
    fn_ohos_free_string_t free_fn = g_fns.free_string;
    if (uuid != nullptr && free_fn != nullptr) {
        free_fn(uuid);
    }
    napi_value out = nullptr;
    if (napi_create_string_utf8(env, result.c_str(), result.size(), &out) != napi_ok) {
        return ThrowErr(env, "sessionNew: failed to create result string");
    }
    return out;
}

static napi_value NapiVersion(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_version");
    if (fn == nullptr) {
        return nullptr;
    }
    const char* v = reinterpret_cast<fn_ohos_version_t>(fn)();
    std::string result = v != nullptr ? v : "";
    napi_value out = nullptr;
    if (napi_create_string_utf8(env, result.c_str(), result.size(), &out) != napi_ok) {
        return ThrowErr(env, "version: failed to create result string");
    }
    return out;
}

static napi_value NapiGetOption(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string key;
    if (!GetArgStr(env, args.argv[0], &key)) {
        return ThrowErr(env, "getOption: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_get_option");
    if (fn == nullptr) {
        return nullptr;
    }
    char* value = reinterpret_cast<fn_ohos_get_option_t>(fn)(key.c_str());
    std::string result = value != nullptr ? value : "";
    fn_ohos_free_string_t free_fn = g_fns.free_string;
    if (value != nullptr && free_fn != nullptr) {
        free_fn(value);
    }
    napi_value out = nullptr;
    if (napi_create_string_utf8(env, result.c_str(), result.size(), &out) != napi_ok) {
        return ThrowErr(env, "getOption: failed to create result string");
    }
    return out;
}

static napi_value NapiSetOption(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string key;
    std::string value;
    if (!GetArgStr(env, args.argv[0], &key) || !GetArgStr(env, args.argv[1], &value)) {
        return ThrowErr(env, "setOption: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_set_option");
    if (fn == nullptr) {
        return nullptr;
    }
    int32_t rc = reinterpret_cast<fn_ohos_set_option_t>(fn)(key.c_str(), value.c_str());
    return MakeInt32(env, rc);
}

// Shared body for the (uuid, ...) -> int family. argCount includes uuid.
template <typename Binder>
static napi_value CallIntWithUuid(napi_env env, napi_callback_info info, const char* symbol,
                                  const Binder& binder) {
    (void)info;
    void* fn = RequireFn(env, symbol);
    if (fn == nullptr) {
        return nullptr;
    }
    int32_t rc = binder(reinterpret_cast<void*>(fn));
    return MakeInt32(env, rc);
}

static napi_value NapiSessionStart(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "sessionStart: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_start",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_start_t>(fn)(uuid.c_str());
                           });
}

static napi_value NapiSessionLogin(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 3, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string password;
    bool remember = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &password) ||
        !GetArgBool(env, args.argv[2], &remember)) {
        return ThrowErr(env, "sessionLogin: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_login",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_login_t>(fn)(
                                   uuid.c_str(), password.c_str(), remember ? 1 : 0);
                           });
}

static napi_value NapiSessionSend2fa(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 3, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string code;
    bool trust = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &code) ||
        !GetArgBool(env, args.argv[2], &trust)) {
        return ThrowErr(env, "sessionSend2fa: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_send2fa",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_send2fa_t>(fn)(
                                   uuid.c_str(), code.c_str(), trust ? 1 : 0);
                           });
}

static napi_value NapiSessionReconnect(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    bool force_relay = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgBool(env, args.argv[1], &force_relay)) {
        return ThrowErr(env, "sessionReconnect: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_reconnect",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_reconnect_t>(fn)(
                                   uuid.c_str(), force_relay ? 1 : 0);
                           });
}

static napi_value NapiSessionClose(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "sessionClose: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_close",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_close_t>(fn)(uuid.c_str());
                           });
}

static napi_value NapiSendMouse(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 8, &args)) {
        return nullptr;
    }
    std::string uuid;
    int32_t mask = 0;
    int32_t x = 0;
    int32_t y = 0;
    bool alt = false;
    bool ctrl = false;
    bool shift = false;
    bool command = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &mask) ||
        !GetArgInt32(env, args.argv[2], &x) || !GetArgInt32(env, args.argv[3], &y) ||
        !GetArgBool(env, args.argv[4], &alt) || !GetArgBool(env, args.argv[5], &ctrl) ||
        !GetArgBool(env, args.argv[6], &shift) || !GetArgBool(env, args.argv[7], &command)) {
        return ThrowErr(env, "sendMouse: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_send_mouse",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_send_mouse_t>(fn)(
                                   uuid.c_str(), mask, x, y, alt ? 1 : 0, ctrl ? 1 : 0,
                                   shift ? 1 : 0, command ? 1 : 0);
                           });
}

static napi_value NapiInputKey(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 8, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string name;
    bool down = false;
    bool press = false;
    bool alt = false;
    bool ctrl = false;
    bool shift = false;
    bool command = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &name) ||
        !GetArgBool(env, args.argv[2], &down) || !GetArgBool(env, args.argv[3], &press) ||
        !GetArgBool(env, args.argv[4], &alt) || !GetArgBool(env, args.argv[5], &ctrl) ||
        !GetArgBool(env, args.argv[6], &shift) || !GetArgBool(env, args.argv[7], &command)) {
        return ThrowErr(env, "inputKey: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_input_key",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_input_key_t>(fn)(
                                   uuid.c_str(), name.c_str(), down ? 1 : 0, press ? 1 : 0,
                                   alt ? 1 : 0, ctrl ? 1 : 0, shift ? 1 : 0, command ? 1 : 0);
                           });
}

static napi_value NapiInputString(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string value;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &value)) {
        return ThrowErr(env, "inputString: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_input_string",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_input_string_t>(fn)(uuid.c_str(),
                                                                                    value.c_str());
                           });
}

static napi_value NapiRefreshVideo(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "refreshVideo: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_refresh",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_refresh_t>(fn)(uuid.c_str());
                           });
}

static napi_value NapiSwitchDisplay(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    int32_t display = 0;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &display)) {
        return ThrowErr(env, "switchDisplay: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_switch_display",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_switch_display_t>(fn)(uuid.c_str(),
                                                                                     display);
                           });
}

static napi_value NapiSendClipboard(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string text;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &text)) {
        return ThrowErr(env, "sendClipboard: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_send_clipboard",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_send_clipboard_t>(fn)(uuid.c_str(),
                                                                                     text.c_str());
                           });
}

static napi_value NapiSendFiles(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 7, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string path;
    std::string to;
    int32_t act_id = 0;
    int32_t file_num = 0;
    bool include_hidden = false;
    bool is_remote = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &act_id) ||
        !GetArgStr(env, args.argv[2], &path) || !GetArgStr(env, args.argv[3], &to) ||
        !GetArgInt32(env, args.argv[4], &file_num) ||
        !GetArgBool(env, args.argv[5], &include_hidden) ||
        !GetArgBool(env, args.argv[6], &is_remote)) {
        return ThrowErr(env, "sendFiles: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_send_files",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_send_files_t>(fn)(
                                   uuid.c_str(), act_id, path.c_str(), to.c_str(), file_num,
                                   include_hidden ? 1 : 0, is_remote ? 1 : 0);
                           });
}

static napi_value NapiSetConfirmOverrideFile(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 6, &args)) {
        return nullptr;
    }
    std::string uuid;
    int32_t act_id = 0;
    int32_t file_num = 0;
    bool need_override = false;
    bool remember = false;
    bool is_upload = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &act_id) ||
        !GetArgInt32(env, args.argv[2], &file_num) ||
        !GetArgBool(env, args.argv[3], &need_override) ||
        !GetArgBool(env, args.argv[4], &remember) || !GetArgBool(env, args.argv[5], &is_upload)) {
        return ThrowErr(env, "setConfirmOverrideFile: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_set_confirm_override_file",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_set_confirm_override_file_t>(fn)(
                                   uuid.c_str(), act_id, file_num, need_override ? 1 : 0,
                                   remember ? 1 : 0, is_upload ? 1 : 0);
                           });
}

static napi_value NapiCancelJob(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    int32_t act_id = 0;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &act_id)) {
        return ThrowErr(env, "cancelJob: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_cancel_job",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_cancel_job_t>(fn)(uuid.c_str(),
                                                                                 act_id);
                           });
}

static napi_value NapiRemoveFile(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 5, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string path;
    int32_t act_id = 0;
    int32_t file_num = 0;
    bool is_remote = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &act_id) ||
        !GetArgStr(env, args.argv[2], &path) || !GetArgInt32(env, args.argv[3], &file_num) ||
        !GetArgBool(env, args.argv[4], &is_remote)) {
        return ThrowErr(env, "removeFile: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_remove_file",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_remove_file_t>(fn)(
                                   uuid.c_str(), act_id, path.c_str(), file_num,
                                   is_remote ? 1 : 0);
                           });
}

static napi_value NapiReadDir(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 4, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string path;
    bool include_hidden = false;
    bool is_remote = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &path) ||
        !GetArgBool(env, args.argv[2], &include_hidden) ||
        !GetArgBool(env, args.argv[3], &is_remote)) {
        return ThrowErr(env, "readDir: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_read_dir",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_read_dir_t>(fn)(
                                   uuid.c_str(), path.c_str(), include_hidden ? 1 : 0,
                                   is_remote ? 1 : 0);
                           });
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

static napi_value NapiRecordScreen(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    bool start = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgBool(env, args.argv[1], &start)) {
        return ThrowErr(env, "recordScreen: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_record_screen",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_record_screen_t>(fn)(
                                   uuid.c_str(), start ? 1 : 0);
                           });
}

static napi_value NapiRequestVoiceCall(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "requestVoiceCall: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_request_voice_call",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_request_voice_call_t>(fn)(
                                   uuid.c_str());
                           });
}

static napi_value NapiCloseVoiceCall(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "closeVoiceCall: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_close_voice_call",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_close_voice_call_t>(fn)(
                                   uuid.c_str());
                           });
}

static napi_value NapiSendClipboardImagePng(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string png_base64;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &png_base64)) {
        return ThrowErr(env, "sendClipboardImagePng: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_send_clipboard_image_png",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_send_clipboard_image_png_t>(fn)(
                                   uuid.c_str(), png_base64.c_str());
                           });
}

// Display options: quality / codec preference / view-only. String returns
// follow the getOption pattern (caller frees via ohos_free_string).
static napi_value NapiSessionGetOption(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string key;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &key)) {
        return ThrowErr(env, "sessionGetOption: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_session_get_option");
    if (fn == nullptr) {
        return nullptr;
    }
    char* value = reinterpret_cast<fn_ohos_session_get_option_t>(fn)(uuid.c_str(), key.c_str());
    std::string result = value != nullptr ? value : "";
    fn_ohos_free_string_t free_fn = g_fns.free_string;
    if (value != nullptr && free_fn != nullptr) {
        free_fn(value);
    }
    napi_value out = nullptr;
    if (napi_create_string_utf8(env, result.c_str(), result.size(), &out) != napi_ok) {
        return ThrowErr(env, "sessionGetOption: failed to create result string");
    }
    return out;
}

static napi_value NapiSessionSetOption(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 3, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string key;
    std::string value;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &key) ||
        !GetArgStr(env, args.argv[2], &value)) {
        return ThrowErr(env, "sessionSetOption: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_session_set_option",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_session_set_option_t>(fn)(
                                   uuid.c_str(), key.c_str(), value.c_str());
                           });
}

static napi_value NapiSaveImageQuality(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string value;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &value)) {
        return ThrowErr(env, "saveImageQuality: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_save_image_quality",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_save_image_quality_t>(fn)(
                                   uuid.c_str(), value.c_str());
                           });
}

static napi_value NapiSaveCustomImageQuality(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    int32_t quality = 0;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &quality)) {
        return ThrowErr(env, "saveCustomImageQuality: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_save_custom_image_quality",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_save_custom_image_quality_t>(fn)(
                                   uuid.c_str(), quality);
                           });
}

static napi_value NapiSetCustomFps(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    int32_t fps = 0;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgInt32(env, args.argv[1], &fps)) {
        return ThrowErr(env, "setCustomFps: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_set_custom_fps",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_set_custom_fps_t>(fn)(uuid.c_str(),
                                                                                     fps);
                           });
}

static napi_value NapiToggleOption(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string name;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &name)) {
        return ThrowErr(env, "toggleOption: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_toggle_option",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_toggle_option_t>(fn)(uuid.c_str(),
                                                                                    name.c_str());
                           });
}

static napi_value NapiGetToggleOption(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    std::string name;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgStr(env, args.argv[1], &name)) {
        return ThrowErr(env, "getToggleOption: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_get_toggle_option",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_get_toggle_option_t>(fn)(
                                   uuid.c_str(), name.c_str());
                           });
}

static napi_value NapiUpdateSupportedDecodings(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "updateSupportedDecodings: invalid arguments");
    }
    return CallIntWithUuid(env, info, "ohos_update_supported_decodings",
                           [&](void* fn) -> int32_t {
                               return reinterpret_cast<fn_ohos_update_supported_decodings_t>(fn)(
                                   uuid.c_str());
                           });
}

static napi_value NapiAlternativeCodecs(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    std::string uuid;
    if (!GetArgStr(env, args.argv[0], &uuid)) {
        return ThrowErr(env, "alternativeCodecs: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_alternative_codecs");
    if (fn == nullptr) {
        return nullptr;
    }
    char* value = reinterpret_cast<fn_ohos_alternative_codecs_t>(fn)(uuid.c_str());
    std::string result = value != nullptr ? value : "";
    fn_ohos_free_string_t free_fn = g_fns.free_string;
    if (value != nullptr && free_fn != nullptr) {
        free_fn(value);
    }
    napi_value out = nullptr;
    if (napi_create_string_utf8(env, result.c_str(), result.size(), &out) != napi_ok) {
        return ThrowErr(env, "alternativeCodecs: failed to create result string");
    }
    return out;
}

// ---------------------------------------------------------------------------
// Controlled side (controlled peer)
// ---------------------------------------------------------------------------

static napi_value NapiServerStart(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_server_start");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_server_start_t>(fn)());
}

static napi_value NapiServerStartService(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_server_start_service");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_server_start_service_t>(fn)());
}

static napi_value NapiServerStopService(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_server_stop_service");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_server_stop_service_t>(fn)());
}

// Shared body for the zero-argument string getters (frees via ohos_free_string).
static napi_value CallStringNoArgs(napi_env env, const char* symbol) {
    void* fn = RequireFn(env, symbol);
    if (fn == nullptr) {
        return nullptr;
    }
    char* value = reinterpret_cast<fn_ohos_get_server_id_t>(fn)();
    std::string result = value != nullptr ? value : "";
    fn_ohos_free_string_t free_fn = g_fns.free_string;
    if (value != nullptr && free_fn != nullptr) {
        free_fn(value);
    }
    napi_value out = nullptr;
    if (napi_create_string_utf8(env, result.c_str(), result.size(), &out) != napi_ok) {
        return ThrowErr(env, "string getter: failed to create result string");
    }
    return out;
}

static napi_value NapiGetServerId(napi_env env, napi_callback_info info) {
    (void)info;
    return CallStringNoArgs(env, "ohos_get_server_id");
}

static napi_value NapiGetServerPassword(napi_env env, napi_callback_info info) {
    (void)info;
    return CallStringNoArgs(env, "ohos_get_server_password");
}

static napi_value NapiUpdateServerPassword(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_update_server_password");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_update_server_password_t>(fn)());
}

static napi_value NapiHasPermanentPassword(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_has_permanent_password");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_has_permanent_password_t>(fn)());
}

static napi_value NapiServerAuthorize(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    bool accept = false;
    if (!GetArgInt32(env, args.argv[0], &id) || !GetArgBool(env, args.argv[1], &accept)) {
        return ThrowErr(env, "serverAuthorize: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_server_authorize");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_server_authorize_t>(fn)(id, accept ? 1 : 0));
}

static napi_value NapiServerCloseConnection(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    if (!GetArgInt32(env, args.argv[0], &id)) {
        return ThrowErr(env, "serverCloseConnection: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_server_close_connection");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_server_close_connection_t>(fn)(id));
}

static napi_value NapiAnswerVoiceCall(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string uuid;
    bool accept = false;
    if (!GetArgStr(env, args.argv[0], &uuid) || !GetArgBool(env, args.argv[1], &accept)) {
        return ThrowErr(env, "answerVoiceCall: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_answer_voice_call");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(
        env, reinterpret_cast<fn_ohos_answer_voice_call_t>(fn)(uuid.c_str(), accept ? 1 : 0));
}

static napi_value NapiCmHandleIncomingVoiceCall(napi_env env, napi_callback_info info) {    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    bool accept = false;
    if (!GetArgInt32(env, args.argv[0], &id) || !GetArgBool(env, args.argv[1], &accept)) {
        return ThrowErr(env, "cmHandleIncomingVoiceCall: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_cm_handle_incoming_voice_call");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(
        env, reinterpret_cast<fn_ohos_cm_handle_incoming_voice_call_t>(fn)(id, accept ? 1 : 0));
}

static napi_value NapiCmCloseVoiceCall(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    if (!GetArgInt32(env, args.argv[0], &id)) {
        return ThrowErr(env, "cmCloseVoiceCall: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_cm_close_voice_call");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_cm_close_voice_call_t>(fn)(id));
}

static napi_value NapiCmRequestVoiceCall(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    if (!GetArgInt32(env, args.argv[0], &id)) {
        return ThrowErr(env, "cmRequestVoiceCall: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_cm_request_voice_call");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_cm_request_voice_call_t>(fn)(id));
}

static napi_value NapiServerSwitchPermission(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 3, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    std::string name;
    bool enabled = false;
    if (!GetArgInt32(env, args.argv[0], &id) || !GetArgStr(env, args.argv[1], &name) ||
        !GetArgBool(env, args.argv[2], &enabled)) {
        return ThrowErr(env, "serverSwitchPermission: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_server_switch_permission");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_server_switch_permission_t>(fn)(
                              id, name.c_str(), enabled ? 1 : 0));
}

static napi_value NapiServerSendChat(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    int32_t id = 0;
    std::string text;
    if (!GetArgInt32(env, args.argv[0], &id) || !GetArgStr(env, args.argv[1], &text)) {
        return ThrowErr(env, "serverSendChat: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_server_send_chat");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(
        env, reinterpret_cast<fn_ohos_server_send_chat_t>(fn)(id, text.c_str()));
}

static napi_value NapiServerPushClipboard(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    std::string text;
    std::string html;
    if (!GetArgStr(env, args.argv[0], &text) || !GetArgStr(env, args.argv[1], &html)) {
        return ThrowErr(env, "serverPushClipboard: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_server_push_clipboard");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(
        env, reinterpret_cast<fn_ohos_server_push_clipboard_t>(fn)(text.c_str(), html.c_str()));
}

static napi_value NapiCaptureStart(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 2, &args)) {
        return nullptr;
    }
    int32_t width = 0;
    int32_t height = 0;
    if (!GetArgInt32(env, args.argv[0], &width) || !GetArgInt32(env, args.argv[1], &height)) {
        return ThrowErr(env, "captureStart: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_capture_start");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_capture_start_t>(fn)(width, height));
}

static napi_value NapiCaptureStop(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_capture_stop");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_capture_stop_t>(fn)());
}

static napi_value NapiCaptureMic(napi_env env, napi_callback_info info) {
    CallArgs args;
    if (!GetCallArgs(env, info, 1, &args)) {
        return nullptr;
    }
    bool enable = false;
    if (!GetArgBool(env, args.argv[0], &enable)) {
        return ThrowErr(env, "captureMic: invalid arguments");
    }
    void* fn = RequireFn(env, "ohos_capture_mic");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_capture_mic_t>(fn)(enable ? 1 : 0));
}

static napi_value NapiCaptureRunning(napi_env env, napi_callback_info info) {
    (void)info;
    void* fn = RequireFn(env, "ohos_capture_running");
    if (fn == nullptr) {
        return nullptr;
    }
    return MakeInt32(env, reinterpret_cast<fn_ohos_capture_running_t>(fn)());
}

EXTERN_C_START
static napi_value EntryInit(napi_env env, napi_value exports) {
    struct Prop {
        const char* name;
        napi_callback fn;
    };
    const Prop props[] = {
        {"init", NapiInit},
        {"setCallbacks", NapiSetCallbacks},
        {"initVideo", NapiInitVideo},
        {"sessionNew", NapiSessionNew},
        {"sessionStart", NapiSessionStart},
        {"sessionLogin", NapiSessionLogin},
        {"sessionSend2fa", NapiSessionSend2fa},
        {"sessionReconnect", NapiSessionReconnect},
        {"sessionClose", NapiSessionClose},
        {"sendMouse", NapiSendMouse},
        {"inputKey", NapiInputKey},
        {"inputString", NapiInputString},
        {"refreshVideo", NapiRefreshVideo},
        {"switchDisplay", NapiSwitchDisplay},
        {"sendClipboard", NapiSendClipboard},
        {"sendFiles", NapiSendFiles},
        {"setConfirmOverrideFile", NapiSetConfirmOverrideFile},
        {"cancelJob", NapiCancelJob},
        {"removeFile", NapiRemoveFile},
        {"readDir", NapiReadDir},
        {"getOption", NapiGetOption},
        {"setOption", NapiSetOption},
        {"recordScreen", NapiRecordScreen},
        {"requestVoiceCall", NapiRequestVoiceCall},
        {"closeVoiceCall", NapiCloseVoiceCall},
        {"sendClipboardImagePng", NapiSendClipboardImagePng},
        {"sessionGetOption", NapiSessionGetOption},
        {"sessionSetOption", NapiSessionSetOption},
        {"saveImageQuality", NapiSaveImageQuality},
        {"saveCustomImageQuality", NapiSaveCustomImageQuality},
        {"setCustomFps", NapiSetCustomFps},
        {"toggleOption", NapiToggleOption},
        {"getToggleOption", NapiGetToggleOption},
        {"updateSupportedDecodings", NapiUpdateSupportedDecodings},
        {"alternativeCodecs", NapiAlternativeCodecs},
        {"version", NapiVersion},
        {"serverStart", NapiServerStart},
        {"serverStartService", NapiServerStartService},
        {"serverStopService", NapiServerStopService},
        {"getServerId", NapiGetServerId},
        {"getServerPassword", NapiGetServerPassword},
        {"updateServerPassword", NapiUpdateServerPassword},
        {"hasPermanentPassword", NapiHasPermanentPassword},
        {"serverAuthorize", NapiServerAuthorize},
        {"serverCloseConnection", NapiServerCloseConnection},
        {"cmHandleIncomingVoiceCall", NapiCmHandleIncomingVoiceCall},
        {"cmCloseVoiceCall", NapiCmCloseVoiceCall},
        {"cmRequestVoiceCall", NapiCmRequestVoiceCall},
        {"answerVoiceCall", NapiAnswerVoiceCall},
        {"serverSwitchPermission", NapiServerSwitchPermission},
        {"serverSendChat", NapiServerSendChat},
        {"serverPushClipboard", NapiServerPushClipboard},
        {"captureStart", NapiCaptureStart},
        {"captureStop", NapiCaptureStop},
        {"captureMic", NapiCaptureMic},
        {"captureRunning", NapiCaptureRunning},
    };
    for (const Prop& p : props) {
        napi_value fn = nullptr;
        if (napi_create_function(env, p.name, NAPI_AUTO_LENGTH, p.fn, nullptr, &fn) != napi_ok) {
            return nullptr;
        }
        if (napi_set_named_property(env, exports, p.name, fn) != napi_ok) {
            return nullptr;
        }
    }
    return exports;
}
EXTERN_C_END

static napi_module g_rustdesk_module = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = EntryInit,
    .nm_modname = "entry",
    .nm_priv = nullptr,
    .reserved = {0},
};

extern "C" __attribute__((constructor)) void RegisterRustdeskEntryModule(void) {
    napi_module_register(&g_rustdesk_module);
}
