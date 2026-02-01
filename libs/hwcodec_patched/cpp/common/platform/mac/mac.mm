#include <AVFoundation/AVFoundation.h>
#include <CoreFoundation/CoreFoundation.h>
#include <CoreMedia/CoreMedia.h>
#include <MacTypes.h>
#include <VideoToolbox/VideoToolbox.h>
#include <cstdlib>
#include <pthread.h>
#include <ratio>
#include <sys/_types/_int32_t.h>
#include <sys/event.h>
#include <unistd.h>
#include "../../log.h"

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

// ---------------------- Core: More Robust Hardware Encoder Detection ----------------------
static int32_t hasHardwareEncoder(bool h265) {
    CMVideoCodecType codecType = h265 ? kCMVideoCodecType_HEVC : kCMVideoCodecType_H264;

    // ---------- Path A: Quick Query with Enable + Require ----------
    // Note: Require implies Enable, but setting both here makes it easier to bypass the strategy on some models that default to a software encoder.
    CFMutableDictionaryRef spec = CFDictionaryCreateMutable(kCFAllocatorDefault, 0,
                                                            &kCFTypeDictionaryKeyCallBacks,
                                                            &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(spec, kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder, kCFBooleanTrue);
    CFDictionarySetValue(spec, kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder, kCFBooleanTrue);

    CFDictionaryRef properties = NULL;
    CFStringRef outID = NULL;

    // Use 1280x720 for capability detection to reduce the probability of "no hardware encoding" due to resolution/level issues.
    OSStatus result = VTCopySupportedPropertyDictionaryForEncoder(1280, 720, codecType, spec, &outID, &properties);

    if (properties) CFRelease(properties);
    if (outID) CFRelease(outID);
    if (spec) CFRelease(spec);

    if (result == noErr) {
        // Explicitly found an encoder that meets the "hardware-only" specification.
        return 1;
    }
    // Reaching here means either no encoder satisfying Require was found (common), or another error occurred.
    // For all failure cases, continue with the safer "session-level confirmation" path to avoid misjudgment.

    // ---------- Path B: Create Session and Read UsingHardwareAcceleratedVideoEncoder ----------
    CFMutableDictionaryRef enableOnly = CFDictionaryCreateMutable(kCFAllocatorDefault, 0,
                                                                  &kCFTypeDictionaryKeyCallBacks,
                                                                  &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(enableOnly, kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder, kCFBooleanTrue);

    VTCompressionSessionRef session = NULL;
    // Also use 1280x720 to reduce profile/level interference
    OSStatus st = VTCompressionSessionCreate(kCFAllocatorDefault,
                                             1280, 720, codecType,
                                             enableOnly,      /* encoderSpecification */
                                             NULL,            /* sourceImageBufferAttributes */
                                             NULL,            /* compressedDataAllocator */
                                             NULL,            /* outputCallback */
                                             NULL,            /* outputRefCon */
                                             &session);
    if (enableOnly) CFRelease(enableOnly);

    if (st != noErr || !session) {
        // Creation failed, considered no hardware available.
        return 0;
    }

    // First, explicitly prepare the encoding process to give VideoToolbox a chance to choose between software/hardware.
    OSStatus prepareStatus = VTCompressionSessionPrepareToEncodeFrames(session);
    if (prepareStatus != noErr) {
        VTCompressionSessionInvalidate(session);
        CFRelease(session);
        return 0;
    }

    // Query the session's read-only property: whether it is using a hardware encoder.
    CFBooleanRef usingHW = NULL;
    st = VTSessionCopyProperty(session,
                               kVTCompressionPropertyKey_UsingHardwareAcceleratedVideoEncoder,
                               kCFAllocatorDefault,
                               (void **)&usingHW);

    Boolean isHW = (st == noErr && usingHW && CFBooleanGetValue(usingHW));

    if (usingHW) CFRelease(usingHW);
    VTCompressionSessionInvalidate(session);
    CFRelease(session);

    return isHW ? 1 : 0;
}

// -------------- Your Public Interface: Unchanged ------------------
extern "C" void checkVideoToolboxSupport(int32_t *h264Encoder, int32_t *h265Encoder, int32_t *h264Decoder, int32_t *h265Decoder) {
    // https://stackoverflow.com/questions/50956097/determine-if-ios-device-can-support-hevc-encoding
    *h264Encoder = 0; // H.264 encoder support is disabled due to frequent reliability issues (see encode.rs)
    *h265Encoder = hasHardwareEncoder(true);

    *h264Decoder = VTIsHardwareDecodeSupported(kCMVideoCodecType_H264);
    *h265Decoder = VTIsHardwareDecodeSupported(kCMVideoCodecType_HEVC);

    return;
}

extern "C" uint64_t GetHwcodecGpuSignature() {
    int32_t h264Encoder = 0;
    int32_t h265Encoder = 0;
    int32_t h264Decoder = 0;
    int32_t h265Decoder = 0;
    checkVideoToolboxSupport(&h264Encoder, &h265Encoder, &h264Decoder, &h265Decoder);
    return (uint64_t)h264Encoder << 24 | (uint64_t)h265Encoder << 16 | (uint64_t)h264Decoder << 8 | (uint64_t)h265Decoder;
}

static void *parent_death_monitor_thread(void *arg) {
  int kq = (intptr_t)arg;
  struct kevent events[1];

  int ret = kevent(kq, NULL, 0, events, 1, NULL);

  if (ret > 0) {
    // Parent process died, terminate this process
    LOG_INFO("Parent process died, terminating hwcodec check process");
    exit(1);
  }

  return NULL;
}

extern "C" int setup_parent_death_signal() {
  // On macOS, use kqueue to monitor parent process death
  pid_t parent_pid = getppid();
  int kq = kqueue();

  if (kq == -1) {
    LOG_DEBUG("Failed to create kqueue for parent monitoring");
    return -1;
  }

  struct kevent event;
  EV_SET(&event, parent_pid, EVFILT_PROC, EV_ADD | EV_ONESHOT, NOTE_EXIT, 0,
         NULL);

  int ret = kevent(kq, &event, 1, NULL, 0, NULL);

  if (ret == -1) {
    LOG_ERROR("Failed to register parent death monitoring on macOS\n");
    close(kq);
    return -1;
  } else {

    // Spawn a thread to monitor parent death
    pthread_t monitor_thread;
    ret = pthread_create(&monitor_thread, NULL, parent_death_monitor_thread,
                         (void *)(intptr_t)kq);

    if (ret != 0) {
      LOG_ERROR("Failed to create parent death monitor thread");
      close(kq);
      return -1;
    }

    // Detach the thread so it can run independently
    pthread_detach(monitor_thread);
    return 0;
  }
}
