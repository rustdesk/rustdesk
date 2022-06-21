#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct wire_uint_8_list {
  uint8_t *ptr;
  int32_t len;
} wire_uint_8_list;

typedef struct WireSyncReturnStruct {
  uint8_t *ptr;
  int32_t len;
  bool success;
} WireSyncReturnStruct;

typedef int64_t DartPort;

typedef bool (*DartPostCObjectFnType)(DartPort port_id, void *message);

void wire_rustdesk_core_main(int64_t port_);

void wire_start_global_event_stream(int64_t port_);

void wire_session_connect(int64_t port_, struct wire_uint_8_list *id, bool is_file_transfer);

void wire_get_session_remember(int64_t port_, struct wire_uint_8_list *id);

void wire_get_session_toggle_option(int64_t port_,
                                    struct wire_uint_8_list *id,
                                    struct wire_uint_8_list *arg);

struct WireSyncReturnStruct wire_get_session_toggle_option_sync(struct wire_uint_8_list *id,
                                                                struct wire_uint_8_list *arg);

void wire_get_session_image_quality(int64_t port_, struct wire_uint_8_list *id);

void wire_get_session_option(int64_t port_,
                             struct wire_uint_8_list *id,
                             struct wire_uint_8_list *arg);

void wire_session_login(int64_t port_,
                        struct wire_uint_8_list *id,
                        struct wire_uint_8_list *password,
                        bool remember);

void wire_session_close(int64_t port_, struct wire_uint_8_list *id);

void wire_session_refresh(int64_t port_, struct wire_uint_8_list *id);

void wire_session_reconnect(int64_t port_, struct wire_uint_8_list *id);

void wire_session_toggle_option(int64_t port_,
                                struct wire_uint_8_list *id,
                                struct wire_uint_8_list *value);

void wire_session_set_image_quality(int64_t port_,
                                    struct wire_uint_8_list *id,
                                    struct wire_uint_8_list *value);

void wire_session_lock_screen(int64_t port_, struct wire_uint_8_list *id);

void wire_session_ctrl_alt_del(int64_t port_, struct wire_uint_8_list *id);

void wire_session_switch_display(int64_t port_, struct wire_uint_8_list *id, int32_t value);

void wire_session_input_key(int64_t port_,
                            struct wire_uint_8_list *id,
                            struct wire_uint_8_list *name,
                            bool down,
                            bool press,
                            bool alt,
                            bool ctrl,
                            bool shift,
                            bool command);

void wire_session_input_string(int64_t port_,
                               struct wire_uint_8_list *id,
                               struct wire_uint_8_list *value);

void wire_session_send_chat(int64_t port_,
                            struct wire_uint_8_list *id,
                            struct wire_uint_8_list *text);

void wire_session_send_mouse(int64_t port_,
                             struct wire_uint_8_list *id,
                             int32_t mask,
                             int32_t x,
                             int32_t y,
                             bool alt,
                             bool ctrl,
                             bool shift,
                             bool command);

void wire_session_peer_option(int64_t port_,
                              struct wire_uint_8_list *id,
                              struct wire_uint_8_list *name,
                              struct wire_uint_8_list *value);

void wire_session_input_os_password(int64_t port_,
                                    struct wire_uint_8_list *id,
                                    struct wire_uint_8_list *value);

void wire_session_read_remote_dir(int64_t port_,
                                  struct wire_uint_8_list *id,
                                  struct wire_uint_8_list *path,
                                  bool include_hidden);

void wire_session_send_files(int64_t port_,
                             struct wire_uint_8_list *id,
                             int32_t act_id,
                             struct wire_uint_8_list *path,
                             struct wire_uint_8_list *to,
                             int32_t file_num,
                             bool include_hidden,
                             bool is_remote);

void wire_session_set_confirm_override_file(int64_t port_,
                                            struct wire_uint_8_list *id,
                                            int32_t act_id,
                                            int32_t file_num,
                                            bool need_override,
                                            bool remember,
                                            bool is_upload);

void wire_session_remove_file(int64_t port_,
                              struct wire_uint_8_list *id,
                              int32_t act_id,
                              struct wire_uint_8_list *path,
                              int32_t file_num,
                              bool is_remote);

void wire_session_read_dir_recursive(int64_t port_,
                                     struct wire_uint_8_list *id,
                                     int32_t act_id,
                                     struct wire_uint_8_list *path,
                                     bool is_remote);

void wire_session_remove_all_empty_dirs(int64_t port_,
                                        struct wire_uint_8_list *id,
                                        int32_t act_id,
                                        struct wire_uint_8_list *path,
                                        bool is_remote);

void wire_session_cancel_job(int64_t port_, struct wire_uint_8_list *id, int32_t act_id);

void wire_session_create_dir(int64_t port_,
                             struct wire_uint_8_list *id,
                             int32_t act_id,
                             struct wire_uint_8_list *path,
                             bool is_remote);

void wire_session_read_local_dir_sync(int64_t port_,
                                      struct wire_uint_8_list *id,
                                      struct wire_uint_8_list *path,
                                      bool show_hidden);

struct wire_uint_8_list *new_uint_8_list(int32_t len);

void free_WireSyncReturnStruct(struct WireSyncReturnStruct val);

void store_dart_post_cobject(DartPostCObjectFnType ptr);

/**
 * FFI for rustdesk core's main entry.
 * Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
 */
bool rustdesk_core_main(void);

static int64_t dummy_method_to_enforce_bundling(void) {
    int64_t dummy_var = 0;
    dummy_var ^= ((int64_t) (void*) wire_rustdesk_core_main);
    dummy_var ^= ((int64_t) (void*) wire_start_global_event_stream);
    dummy_var ^= ((int64_t) (void*) wire_session_connect);
    dummy_var ^= ((int64_t) (void*) wire_get_session_remember);
    dummy_var ^= ((int64_t) (void*) wire_get_session_toggle_option);
    dummy_var ^= ((int64_t) (void*) wire_get_session_toggle_option_sync);
    dummy_var ^= ((int64_t) (void*) wire_get_session_image_quality);
    dummy_var ^= ((int64_t) (void*) wire_get_session_option);
    dummy_var ^= ((int64_t) (void*) wire_session_login);
    dummy_var ^= ((int64_t) (void*) wire_session_close);
    dummy_var ^= ((int64_t) (void*) wire_session_refresh);
    dummy_var ^= ((int64_t) (void*) wire_session_reconnect);
    dummy_var ^= ((int64_t) (void*) wire_session_toggle_option);
    dummy_var ^= ((int64_t) (void*) wire_session_set_image_quality);
    dummy_var ^= ((int64_t) (void*) wire_session_lock_screen);
    dummy_var ^= ((int64_t) (void*) wire_session_ctrl_alt_del);
    dummy_var ^= ((int64_t) (void*) wire_session_switch_display);
    dummy_var ^= ((int64_t) (void*) wire_session_input_key);
    dummy_var ^= ((int64_t) (void*) wire_session_input_string);
    dummy_var ^= ((int64_t) (void*) wire_session_send_chat);
    dummy_var ^= ((int64_t) (void*) wire_session_send_mouse);
    dummy_var ^= ((int64_t) (void*) wire_session_peer_option);
    dummy_var ^= ((int64_t) (void*) wire_session_input_os_password);
    dummy_var ^= ((int64_t) (void*) wire_session_read_remote_dir);
    dummy_var ^= ((int64_t) (void*) wire_session_send_files);
    dummy_var ^= ((int64_t) (void*) wire_session_set_confirm_override_file);
    dummy_var ^= ((int64_t) (void*) wire_session_remove_file);
    dummy_var ^= ((int64_t) (void*) wire_session_read_dir_recursive);
    dummy_var ^= ((int64_t) (void*) wire_session_remove_all_empty_dirs);
    dummy_var ^= ((int64_t) (void*) wire_session_cancel_job);
    dummy_var ^= ((int64_t) (void*) wire_session_create_dir);
    dummy_var ^= ((int64_t) (void*) wire_session_read_local_dir_sync);
    dummy_var ^= ((int64_t) (void*) new_uint_8_list);
    dummy_var ^= ((int64_t) (void*) free_WireSyncReturnStruct);
    dummy_var ^= ((int64_t) (void*) store_dart_post_cobject);
    return dummy_var;
}