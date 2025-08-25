#ifndef SCREEN_CAPTURE_H
#define SCREEN_CAPTURE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Initialize iOS screen capture
void ios_capture_init(void);

// Start screen capture
bool ios_capture_start(void);

// Stop screen capture  
void ios_capture_stop(void);

// Check if capturing
bool ios_capture_is_active(void);

// Get current frame data
// Returns frame size, or 0 if no frame available
// Buffer must be large enough to hold width * height * 4 bytes (RGBA)
uint32_t ios_capture_get_frame(uint8_t* buffer, uint32_t buffer_size, 
                               uint32_t* out_width, uint32_t* out_height);

// Get display info
void ios_capture_get_display_info(uint32_t* width, uint32_t* height);

// Callback for frame updates from native side
typedef void (*frame_callback_t)(const uint8_t* data, uint32_t size, 
                                 uint32_t width, uint32_t height);

// Set frame callback
void ios_capture_set_callback(frame_callback_t callback);

// Show broadcast picker for system-wide capture
void ios_capture_show_broadcast_picker(void);

// Check if broadcasting (system-wide capture)
bool ios_capture_is_broadcasting(void);

// Audio capture control
void ios_capture_set_audio_enabled(bool enable_mic, bool enable_app_audio);

// Audio callback
typedef void (*audio_callback_t)(const uint8_t* data, uint32_t size, bool is_mic);
void ios_capture_set_audio_callback(audio_callback_t callback);

#ifdef __cplusplus
}
#endif

#endif // SCREEN_CAPTURE_H