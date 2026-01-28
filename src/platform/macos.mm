#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#import <IOKit/hidsystem/IOHIDLib.h>
#include <Security/Authorization.h>
#include <Security/AuthorizationTags.h>

#include <CoreGraphics/CoreGraphics.h>
#include <vector>
#include <map>
#include <set>
#include <mutex>
#include <string>

extern "C" bool CanUseNewApiForScreenCaptureCheck() {
    #ifdef NO_InputMonitoringAuthStatus
    return false;
    #else
    NSOperatingSystemVersion version = [[NSProcessInfo processInfo] operatingSystemVersion];
    return version.majorVersion >= 11;
    #endif
}

extern "C" uint32_t majorVersion() {
    NSOperatingSystemVersion version = [[NSProcessInfo processInfo] operatingSystemVersion];
    return version.majorVersion;
}

extern "C" bool IsCanScreenRecording(bool prompt) {
    #ifdef NO_InputMonitoringAuthStatus
    return false;
    #else
    bool res = CGPreflightScreenCaptureAccess();
    if (!res && prompt) {
        CGRequestScreenCaptureAccess();
    }
    return res;
    #endif
}


// https://github.com/codebytere/node-mac-permissions/blob/main/permissions.mm

extern "C" bool InputMonitoringAuthStatus(bool prompt) {
    #ifdef NO_InputMonitoringAuthStatus
    return true;
    #else
    if (floor(NSAppKitVersionNumber) >= NSAppKitVersionNumber10_15) {
        IOHIDAccessType theType = IOHIDCheckAccess(kIOHIDRequestTypeListenEvent);
        NSLog(@"IOHIDCheckAccess = %d, kIOHIDAccessTypeGranted = %d", theType, kIOHIDAccessTypeGranted);
        switch (theType) {
            case kIOHIDAccessTypeGranted:
                return true;
                break;
            case kIOHIDAccessTypeDenied: {
                if (prompt) {
                    NSString *urlString = @"x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";
                    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:urlString]];
                }
                break;
            }
            case kIOHIDAccessTypeUnknown: {
                if (prompt) {
                    bool result = IOHIDRequestAccess(kIOHIDRequestTypeListenEvent);
                    NSLog(@"IOHIDRequestAccess result = %d", result);
                }
                break;
            }
            default:
                break;
        }
    } else {
        return true;
    }
    return false;
    #endif
}

extern "C" bool Elevate(char* process, char** args) {
    AuthorizationRef authRef;
    OSStatus status;

    status = AuthorizationCreate(NULL, kAuthorizationEmptyEnvironment,
                                kAuthorizationFlagDefaults, &authRef);
    if (status != errAuthorizationSuccess) {
        printf("Failed to create AuthorizationRef\n");
        return false;
    }

    AuthorizationItem authItem = {kAuthorizationRightExecute, 0, NULL, 0};
    AuthorizationRights authRights = {1, &authItem};
    AuthorizationFlags flags = kAuthorizationFlagDefaults |
                                kAuthorizationFlagInteractionAllowed |
                                kAuthorizationFlagPreAuthorize |
                                kAuthorizationFlagExtendRights;
    status = AuthorizationCopyRights(authRef, &authRights, kAuthorizationEmptyEnvironment, flags, NULL);
    if (status != errAuthorizationSuccess) {
        printf("Failed to authorize\n");
        return false;
    }

    if (process != NULL) {
        FILE *pipe = NULL;
        status = AuthorizationExecuteWithPrivileges(authRef, process, kAuthorizationFlagDefaults, args, &pipe);
        if (status != errAuthorizationSuccess) {
            printf("Failed to run as root\n");
            AuthorizationFree(authRef, kAuthorizationFlagDefaults);
            return false;
        }
    }

    AuthorizationFree(authRef, kAuthorizationFlagDefaults);
    return true;
}

extern "C" bool MacCheckAdminAuthorization() {
    return Elevate(NULL, NULL);
}

// https://gist.github.com/briankc/025415e25900750f402235dbf1b74e42
extern "C" float BackingScaleFactor(uint32_t display) {
    NSArray<NSScreen *> *screens = [NSScreen screens];
    for (NSScreen *screen in screens) {
        NSDictionary *deviceDescription = [screen deviceDescription];
        NSNumber *screenNumber = [deviceDescription objectForKey:@"NSScreenNumber"];
        CGDirectDisplayID screenDisplayID = [screenNumber unsignedIntValue];
        if (screenDisplayID == display) {
            return [screen backingScaleFactor];
        }
    }
    return 1;
}

// https://github.com/jhford/screenresolution/blob/master/cg_utils.c
// https://github.com/jdoupe/screenres/blob/master/setgetscreen.m

size_t bitDepth(CGDisplayModeRef mode) {
    size_t depth = 0;
    // Deprecated, same display same bpp? 
    // https://stackoverflow.com/questions/8210824/how-to-avoid-cgdisplaymodecopypixelencoding-to-get-bpp
    // https://github.com/libsdl-org/SDL/pull/6628
	CFStringRef pixelEncoding = CGDisplayModeCopyPixelEncoding(mode);	
    // my numerical representation for kIO16BitFloatPixels and kIO32bitFloatPixels	
    // are made up and possibly non-sensical	
    if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(kIO32BitFloatPixels), kCFCompareCaseInsensitive)) {	
        depth = 96;	
    } else if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(kIO64BitDirectPixels), kCFCompareCaseInsensitive)) {	
        depth = 64;	
    } else if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(kIO16BitFloatPixels), kCFCompareCaseInsensitive)) {	
        depth = 48;	
    } else if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(IO32BitDirectPixels), kCFCompareCaseInsensitive)) {	
        depth = 32;	
    } else if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(kIO30BitDirectPixels), kCFCompareCaseInsensitive)) {	
        depth = 30;	
    } else if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(IO16BitDirectPixels), kCFCompareCaseInsensitive)) {	
        depth = 16;	
    } else if (kCFCompareEqualTo == CFStringCompare(pixelEncoding, CFSTR(IO8BitIndexedPixels), kCFCompareCaseInsensitive)) {	
        depth = 8;	
    }	
    CFRelease(pixelEncoding);	
    return depth;	
}

static bool isHiDPIMode(CGDisplayModeRef mode) {
    // Check if the mode is HiDPI by comparing pixel width to width
    // If pixel width is greater than width, it's a HiDPI mode
    return CGDisplayModeGetPixelWidth(mode) > CGDisplayModeGetWidth(mode);
}

CFArrayRef getAllModes(CGDirectDisplayID display) {
    // Create options dictionary to include HiDPI modes
    CFMutableDictionaryRef options = CFDictionaryCreateMutable(
        kCFAllocatorDefault,
        0,
        &kCFTypeDictionaryKeyCallBacks,
        &kCFTypeDictionaryValueCallBacks);
    // Include HiDPI modes
    CFDictionarySetValue(options, kCGDisplayShowDuplicateLowResolutionModes, kCFBooleanTrue);
    CFArrayRef allModes = CGDisplayCopyAllDisplayModes(display, options);
    CFRelease(options);
    return allModes;
}

extern "C" bool MacGetModeNum(CGDirectDisplayID display, uint32_t *numModes) {
    CFArrayRef allModes = getAllModes(display);
    if (allModes == NULL) {
        return false;
    }
    *numModes = CFArrayGetCount(allModes);
    CFRelease(allModes);
    return true;
}

extern "C" bool MacGetModes(CGDirectDisplayID display, uint32_t *widths, uint32_t *heights, bool *hidpis, uint32_t max, uint32_t *numModes) {
    CGDisplayModeRef currentMode = CGDisplayCopyDisplayMode(display);
    if (currentMode == NULL) {
        return false;
    }
    CFArrayRef allModes = getAllModes(display);
    if (allModes == NULL) {
        CGDisplayModeRelease(currentMode);
        return false;
    }
    uint32_t allModeCount = CFArrayGetCount(allModes);
    uint32_t realNum = 0;
    for (uint32_t i = 0; i < allModeCount && realNum < max; i++) {
        CGDisplayModeRef mode = (CGDisplayModeRef)CFArrayGetValueAtIndex(allModes, i);
        if (CGDisplayModeGetRefreshRate(currentMode) == CGDisplayModeGetRefreshRate(mode) &&
            bitDepth(currentMode) == bitDepth(mode)) {
            widths[realNum] = (uint32_t)CGDisplayModeGetWidth(mode);
            heights[realNum] = (uint32_t)CGDisplayModeGetHeight(mode);
            hidpis[realNum] = isHiDPIMode(mode);
            realNum++;
        }
    }
    *numModes = realNum;
    CGDisplayModeRelease(currentMode);
    CFRelease(allModes);
    return true;
}

extern "C" bool MacGetMode(CGDirectDisplayID display, uint32_t *width, uint32_t *height) {
    CGDisplayModeRef mode = CGDisplayCopyDisplayMode(display);
    if (mode == NULL) {
        return false;
    }
    *width = (uint32_t)CGDisplayModeGetWidth(mode);
    *height = (uint32_t)CGDisplayModeGetHeight(mode);
    CGDisplayModeRelease(mode);
    return true;
}

static bool setDisplayToMode(CGDirectDisplayID display, CGDisplayModeRef mode) {
    CGError rc;
    CGDisplayConfigRef config;
    rc = CGBeginDisplayConfiguration(&config);
    if (rc != kCGErrorSuccess) {
        return false;
    }
    rc = CGConfigureDisplayWithDisplayMode(config, display, mode, NULL);
    if (rc != kCGErrorSuccess) {
        return false;
    }
    rc = CGCompleteDisplayConfiguration(config, kCGConfigureForSession);
    if (rc != kCGErrorSuccess) {
        return false;
    }
    return true;
}

// Set the display to a specific mode based on width and height.
// Returns true if the display mode was successfully changed, false otherwise.
// If no such mode is available, it will not change the display mode.
//
// If `tryHiDPI` is true, it will try to set the display to a HiDPI mode if available.
// If no HiDPI mode is available, it will fall back to a non-HiDPI mode with the same resolution.
// If `tryHiDPI` is false, it sets the display to the first mode with the same resolution, no matter if it's HiDPI or not.
extern "C" bool MacSetMode(CGDirectDisplayID display, uint32_t width, uint32_t height, bool tryHiDPI)
{
    bool ret = false;
    CGDisplayModeRef currentMode = CGDisplayCopyDisplayMode(display);
    if (currentMode == NULL) {
        return ret;
    }
    CFArrayRef allModes = getAllModes(display);

    if (allModes == NULL) {
        CGDisplayModeRelease(currentMode);
        return ret;
    }
    int numModes = CFArrayGetCount(allModes);
    CGDisplayModeRef preferredHiDPIMode = NULL;
    CGDisplayModeRef fallbackMode = NULL;
    for (int i = 0; i < numModes; i++) {
        CGDisplayModeRef mode = (CGDisplayModeRef)CFArrayGetValueAtIndex(allModes, i);
        if (width == CGDisplayModeGetWidth(mode) &&
            height == CGDisplayModeGetHeight(mode) && 
            CGDisplayModeGetRefreshRate(currentMode) == CGDisplayModeGetRefreshRate(mode) &&
            bitDepth(currentMode) == bitDepth(mode)) {

            if (isHiDPIMode(mode)) {
                preferredHiDPIMode = mode;
                break;
            } else {
                fallbackMode = mode;
                if (!tryHiDPI) {
                    break;
                }
            }
        }
    }

    if (preferredHiDPIMode) {
        ret = setDisplayToMode(display, preferredHiDPIMode);
    } else if (fallbackMode) {
        ret = setDisplayToMode(display, fallbackMode);
    }

    CGDisplayModeRelease(currentMode);
    CFRelease(allModes);
    return ret;
}

static CFMachPortRef g_eventTap = NULL;
static CFRunLoopSourceRef g_runLoopSource = NULL;
static std::mutex g_privacyModeMutex;
static bool g_privacyModeActive = false;

// Flag to request asynchronous shutdown of privacy mode.
// This is set by DisplayReconfigurationCallback when an error occurs, instead of calling
// TurnOffPrivacyModeInternal() directly from within the callback. This avoids potential
// issues with unregistering a callback from within itself, which is not explicitly
// guaranteed to be safe by Apple documentation.
static bool g_privacyModeShutdownRequested = false;

// Timestamp of the last display reconfiguration event (in milliseconds).
// Used for debouncing rapid successive changes (e.g., multiple resolution changes).
static uint64_t g_lastReconfigTimestamp = 0;

// Flag indicating whether a delayed blackout reapplication is already scheduled.
// Prevents multiple concurrent delayed tasks from being created.
static bool g_blackoutReapplicationScheduled = false;

// Use CFStringRef (UUID) as key instead of CGDirectDisplayID for stability across reconnections
// CGDirectDisplayID can change when displays are reconnected, but UUID remains stable
static std::map<std::string, std::vector<CGGammaValue>> g_originalGammas;

// The event source user data value used by enigo library for injected events.
// This allows us to distinguish remote input (which should be allowed) from local physical input.
// See: libs/enigo/src/macos/macos_impl.rs - ENIGO_INPUT_EXTRA_VALUE
static const int64_t ENIGO_INPUT_EXTRA_VALUE = 100;

// Duration in milliseconds to monitor and enforce blackout after display reconfiguration.
// macOS may restore default gamma (via ColorSync) at unpredictable times after display changes,
// so we need to actively monitor and reapply blackout during this period.
static const int64_t DISPLAY_RECONFIG_MONITOR_DURATION_MS = 5000;

// Interval in milliseconds between gamma checks during the monitoring period.
static const int64_t GAMMA_CHECK_INTERVAL_MS = 200;

// Helper function to get UUID string from DisplayID
static std::string GetDisplayUUID(CGDirectDisplayID displayId) {
    CFUUIDRef uuid = CGDisplayCreateUUIDFromDisplayID(displayId);
    if (uuid == NULL) {
        return "";
    }
    CFStringRef uuidStr = CFUUIDCreateString(kCFAllocatorDefault, uuid);
    CFRelease(uuid);
    if (uuidStr == NULL) {
        return "";
    }
    char buffer[128];
    if (CFStringGetCString(uuidStr, buffer, sizeof(buffer), kCFStringEncodingUTF8)) {
        CFRelease(uuidStr);
        return std::string(buffer);
    }
    CFRelease(uuidStr);
    return "";
}

// Helper function to find DisplayID by UUID from current online displays
static CGDirectDisplayID FindDisplayIdByUUID(const std::string& targetUuid) {
    uint32_t count = 0;
    CGGetOnlineDisplayList(0, NULL, &count);
    if (count == 0) return kCGNullDirectDisplay;
    
    std::vector<CGDirectDisplayID> displays(count);
    CGGetOnlineDisplayList(count, displays.data(), &count);
    
    for (uint32_t i = 0; i < count; i++) {
        std::string uuid = GetDisplayUUID(displays[i]);
        if (uuid == targetUuid) {
            return displays[i];
        }
    }
    return kCGNullDirectDisplay;
}

// Helper function to restore gamma values for all displays in g_originalGammas.
// Returns true if all displays were restored successfully, false if any failed.
// Note: This function does NOT clear g_originalGammas - caller should do that if needed.
static bool RestoreAllGammas() {
    bool allSuccess = true;
    for (auto const& [uuid, gamma] : g_originalGammas) {
        CGDirectDisplayID d = FindDisplayIdByUUID(uuid);
        if (d == kCGNullDirectDisplay) {
            NSLog(@"Display with UUID %s no longer online, skipping gamma restore", uuid.c_str());
            continue;
        }
        
        uint32_t sampleCount = gamma.size() / 3;
        if (sampleCount > 0) {
            const CGGammaValue* red = gamma.data();
            const CGGammaValue* green = red + sampleCount;
            const CGGammaValue* blue = green + sampleCount;
            CGError error = CGSetDisplayTransferByTable(d, sampleCount, red, green, blue);
            if (error != kCGErrorSuccess) {
                NSLog(@"Failed to restore gamma for display (ID: %u, UUID: %s, error: %d)", (unsigned)d, uuid.c_str(), error);
                allSuccess = false;
            }
        }
    }
    return allSuccess;
}

// Helper function to apply blackout to a single display
static bool ApplyBlackoutToDisplay(CGDirectDisplayID display) {
    uint32_t capacity = CGDisplayGammaTableCapacity(display);
    if (capacity > 0) {
        std::vector<CGGammaValue> zeros(capacity, 0.0f);
        CGError error = CGSetDisplayTransferByTable(display, capacity, zeros.data(), zeros.data(), zeros.data());
        if (error != kCGErrorSuccess) {
            NSLog(@"ApplyBlackoutToDisplay: Failed to set gamma for display %u (error %d)", (unsigned)display, error);
            return false;
        }
        return true;
    }
    NSLog(@"ApplyBlackoutToDisplay: Display %u has zero gamma table capacity, blackout not supported", (unsigned)display);
    return false;
}

// Forward declaration - defined later in the file
// Must be called while holding g_privacyModeMutex
static bool TurnOffPrivacyModeInternal();

// Helper function to schedule asynchronous shutdown of privacy mode.
// This is called from DisplayReconfigurationCallback when an error occurs,
// instead of calling TurnOffPrivacyModeInternal() directly. This avoids
// potential issues with unregistering a callback from within itself.
// Note: This function should be called while holding g_privacyModeMutex.
static void ScheduleAsyncPrivacyModeShutdown(const char* reason) {
    if (g_privacyModeShutdownRequested) {
        // Already requested, no need to schedule again
        return;
    }
    g_privacyModeShutdownRequested = true;
    NSLog(@"Privacy mode shutdown requested: %s", reason);
    
    // Schedule the actual shutdown on the main queue asynchronously
    // This ensures we're outside the callback when we unregister it
    dispatch_async(dispatch_get_main_queue(), ^{
        std::lock_guard<std::mutex> lock(g_privacyModeMutex);
        if (g_privacyModeShutdownRequested && g_privacyModeActive) {
            NSLog(@"Executing deferred privacy mode shutdown");
            TurnOffPrivacyModeInternal();
        }
        g_privacyModeShutdownRequested = false;
    });
}

// Helper function to apply blackout to all online displays.
// Must be called while holding g_privacyModeMutex.
static void ApplyBlackoutToAllDisplays() {
    uint32_t onlineCount = 0;
    CGGetOnlineDisplayList(0, NULL, &onlineCount);
    std::vector<CGDirectDisplayID> onlineDisplays(onlineCount);
    CGGetOnlineDisplayList(onlineCount, onlineDisplays.data(), &onlineCount);
    
    for (uint32_t i = 0; i < onlineCount; i++) {
        ApplyBlackoutToDisplay(onlineDisplays[i]);
    }
}

// Helper function to get current timestamp in milliseconds
static uint64_t GetCurrentTimestampMs() {
    return (uint64_t)(CFAbsoluteTimeGetCurrent() * 1000.0);
}

// Helper function to check if a display's gamma is currently blacked out (all zeros).
// Returns true if gamma appears to be blacked out, false otherwise.
static bool IsDisplayBlackedOut(CGDirectDisplayID display) {
    uint32_t capacity = CGDisplayGammaTableCapacity(display);
    if (capacity == 0) {
        return true; // Can't check, assume it's fine
    }
    
    std::vector<CGGammaValue> red(capacity), green(capacity), blue(capacity);
    uint32_t sampleCount = 0;
    if (CGGetDisplayTransferByTable(display, capacity, red.data(), green.data(), blue.data(), &sampleCount) != kCGErrorSuccess) {
        return true; // Can't read, assume it's fine
    }
    
    // Check if all values are zero (or very close to zero)
    for (uint32_t i = 0; i < sampleCount; i++) {
        if (red[i] > 0.01f || green[i] > 0.01f || blue[i] > 0.01f) {
            return false; // Not blacked out
        }
    }
    return true;
}

// Internal function that monitors and enforces blackout for a period after display reconfiguration.
// This function checks gamma values periodically and reapplies blackout if needed.
// Must NOT be called while holding g_privacyModeMutex (it acquires the lock internally).
static void RunBlackoutMonitor() {
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(GAMMA_CHECK_INTERVAL_MS * NSEC_PER_MSEC)), dispatch_get_main_queue(), ^{
        std::lock_guard<std::mutex> lock(g_privacyModeMutex);
        
        if (!g_privacyModeActive) {
            g_blackoutReapplicationScheduled = false;
            return;
        }
        
        uint64_t now = GetCurrentTimestampMs();
        
        // Calculate effective end time based on the last reconfig event
        uint64_t effectiveEndTime = g_lastReconfigTimestamp + DISPLAY_RECONFIG_MONITOR_DURATION_MS;
        
        // Check all displays and reapply blackout if any has been restored
        uint32_t onlineCount = 0;
        CGGetOnlineDisplayList(0, NULL, &onlineCount);
        std::vector<CGDirectDisplayID> onlineDisplays(onlineCount);
        CGGetOnlineDisplayList(onlineCount, onlineDisplays.data(), &onlineCount);
        
        bool needsReapply = false;
        for (uint32_t i = 0; i < onlineCount; i++) {
            if (!IsDisplayBlackedOut(onlineDisplays[i])) {
                needsReapply = true;
                break;
            }
        }
        
        if (needsReapply) {
            NSLog(@"Gamma was restored by system, reapplying blackout");
            ApplyBlackoutToAllDisplays();
        }
        
        // Continue monitoring if we haven't reached the end time
        if (now < effectiveEndTime) {
            RunBlackoutMonitor();
        } else {
            NSLog(@"Blackout monitoring period ended");
            g_blackoutReapplicationScheduled = false;
        }
    });
}

// Helper function to start monitoring and enforcing blackout after display reconfiguration.
// This is used after display reconfiguration events because macOS may restore
// default gamma (via ColorSync) at unpredictable times after display changes.
// Note: This function should be called while holding g_privacyModeMutex.
static void ScheduleDelayedBlackoutReapplication(const char* reason) {
    // Update timestamp to current time
    g_lastReconfigTimestamp = GetCurrentTimestampMs();
    
    NSLog(@"Starting blackout monitor: %s", reason);
    
    // Only schedule if not already scheduled
    if (!g_blackoutReapplicationScheduled) {
        g_blackoutReapplicationScheduled = true;
        RunBlackoutMonitor();
    }
    // If already scheduled, the running monitor will see the updated timestamp
    // and extend its monitoring period
}

// Display reconfiguration callback to handle display connect/disconnect events
//
// IMPORTANT: When errors occur in this callback, we use ScheduleAsyncPrivacyModeShutdown()
// instead of calling TurnOffPrivacyModeInternal() directly. This is because:
// 1. TurnOffPrivacyModeInternal() calls CGDisplayRemoveReconfigurationCallback to unregister
//    this callback, and unregistering a callback from within itself is not explicitly
//    guaranteed to be safe by Apple documentation.
// 2. Using async dispatch ensures we're completely outside the callback context when
//    performing the cleanup, avoiding any potential undefined behavior.
static void DisplayReconfigurationCallback(CGDirectDisplayID display, CGDisplayChangeSummaryFlags flags, void *userInfo) {
    (void)userInfo;
    
    // Note: We need to handle the callback carefully because:
    // 1. macOS may call this callback multiple times during display reconfiguration
    // 2. The system may restore ColorSync settings after our gamma change
    // 3. We should not hold the lock for too long in the callback
    
    // Skip begin configuration flag - wait for the actual change
    if (flags & kCGDisplayBeginConfigurationFlag) {
        return;
    }
    
    std::lock_guard<std::mutex> lock(g_privacyModeMutex);
    
    if (!g_privacyModeActive) {
        return;
    }
    
    if (flags & kCGDisplayAddFlag) {
        // A display was added - apply blackout to it
        NSLog(@"Display %u added during privacy mode, applying blackout", (unsigned)display);
        std::string uuid = GetDisplayUUID(display);
        if (uuid.empty()) {
            NSLog(@"Failed to get UUID for newly added display %u, exiting privacy mode", (unsigned)display);
            ScheduleAsyncPrivacyModeShutdown("Failed to get UUID for newly added display");
            return;
        }
        
        // Save original gamma if not already saved for this UUID
        if (g_originalGammas.find(uuid) == g_originalGammas.end()) {
            uint32_t capacity = CGDisplayGammaTableCapacity(display);
            if (capacity > 0) {
                std::vector<CGGammaValue> red(capacity), green(capacity), blue(capacity);
                uint32_t sampleCount = 0;
                if (CGGetDisplayTransferByTable(display, capacity, red.data(), green.data(), blue.data(), &sampleCount) == kCGErrorSuccess) {
                    std::vector<CGGammaValue> all;
                    all.insert(all.end(), red.begin(), red.begin() + sampleCount);
                    all.insert(all.end(), green.begin(), green.begin() + sampleCount);
                    all.insert(all.end(), blue.begin(), blue.begin() + sampleCount);
                    g_originalGammas[uuid] = all;
                } else {
                    NSLog(@"DisplayReconfigurationCallback: Failed to get gamma table for display %u (UUID: %s), exiting privacy mode", (unsigned)display, uuid.c_str());
                    ScheduleAsyncPrivacyModeShutdown("Failed to get gamma table for newly added display");
                    return;
                }
            } else {
                NSLog(@"DisplayReconfigurationCallback: Display %u (UUID: %s) has zero gamma table capacity, exiting privacy mode", (unsigned)display, uuid.c_str());
                ScheduleAsyncPrivacyModeShutdown("Newly added display has zero gamma table capacity");
                return;
            }
        }
        
        // Apply blackout to the new display immediately
        if (!ApplyBlackoutToDisplay(display)) {
            NSLog(@"DisplayReconfigurationCallback: Failed to blackout display %u (UUID: %s), exiting privacy mode", (unsigned)display, uuid.c_str());
            ScheduleAsyncPrivacyModeShutdown("Failed to blackout newly added display");
            return;
        }
        
        // Schedule a delayed re-application to handle ColorSync restoration
        // macOS may restore default gamma for ALL displays after a new display is added,
        // so we need to reapply blackout to all online displays, not just the new one
        ScheduleDelayedBlackoutReapplication("after new display added");
    } else if (flags & kCGDisplayRemoveFlag) {
        // A display was removed - update our mapping and reapply blackout to remaining displays
        NSLog(@"Display %u removed during privacy mode", (unsigned)display);
        std::string uuid = GetDisplayUUID(display);
        (void)uuid; // UUID retrieved for potential future use or logging
        
        // When a display is removed, macOS may reconfigure other displays and restore their gamma.
        // Schedule a delayed re-application of blackout to all remaining online displays.
        ScheduleDelayedBlackoutReapplication("after display removal");
    } else if (flags & kCGDisplaySetModeFlag) {
        // Display mode changed (resolution change, ColorSync/Night Shift interference, etc.)
        // macOS resets gamma to default when display mode changes, so we need to reapply blackout.
        // Schedule a delayed re-application because ColorSync restoration happens asynchronously.
        NSLog(@"Display %u mode changed during privacy mode, reapplying blackout", (unsigned)display);
        ScheduleDelayedBlackoutReapplication("after display mode change");
    }
}

CGEventRef MyEventTapCallback(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *refcon) {
    (void)proxy;
    (void)refcon;
    
    // Handle EventTap being disabled by system timeout
    if (type == kCGEventTapDisabledByTimeout) {
        NSLog(@"EventTap was disabled by timeout, re-enabling");
        if (g_eventTap) {
            CGEventTapEnable(g_eventTap, true);
        }
        return event;
    }
    
    // Handle EventTap being disabled by user input
    if (type == kCGEventTapDisabledByUserInput) {
        NSLog(@"EventTap was disabled by user input, re-enabling");
        if (g_eventTap) {
            CGEventTapEnable(g_eventTap, true);
        }
        return event;
    }
    
    // Allow events explicitly injected by enigo (remote input), identified via custom user data.
    int64_t userData = CGEventGetIntegerValueField(event, kCGEventSourceUserData);
    if (userData == ENIGO_INPUT_EXTRA_VALUE) {
        return event;
    }
    // Block local physical HID input.
    if (CGEventGetIntegerValueField(event, kCGEventSourceStateID) == kCGEventSourceStateHIDSystemState) {
        return NULL;
    }
    return event;
}

// Helper function to set up EventTap on the main thread
// Returns true if EventTap was successfully created and enabled
static bool SetupEventTapOnMainThread() {
    __block bool success = false;
    
    void (^setupBlock)(void) = ^{
        if (g_eventTap) {
            // Already set up
            success = true;
            return;
        }
        
        // Note: kCGEventTapDisabledByTimeout and kCGEventTapDisabledByUserInput are special
        // notification types (0xFFFFFFFE and 0xFFFFFFFF) that are delivered via the callback's
        // type parameter, not through the event mask. They should NOT be included in eventMask
        // as bit-shifting by these values causes undefined behavior.
        CGEventMask eventMask = (1 << kCGEventKeyDown) | (1 << kCGEventKeyUp) |
                                (1 << kCGEventLeftMouseDown) | (1 << kCGEventLeftMouseUp) |
                                (1 << kCGEventRightMouseDown) | (1 << kCGEventRightMouseUp) |
                                (1 << kCGEventOtherMouseDown) | (1 << kCGEventOtherMouseUp) |
                                (1 << kCGEventLeftMouseDragged) | (1 << kCGEventRightMouseDragged) |
                                (1 << kCGEventOtherMouseDragged) |
                                (1 << kCGEventMouseMoved) | (1 << kCGEventScrollWheel);
        
        g_eventTap = CGEventTapCreate(kCGHIDEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault,
                                      eventMask, MyEventTapCallback, NULL);
        if (g_eventTap) {
            g_runLoopSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, g_eventTap, 0);
            CFRunLoopAddSource(CFRunLoopGetMain(), g_runLoopSource, kCFRunLoopCommonModes);
            CGEventTapEnable(g_eventTap, true);
            success = true;
        } else {
            NSLog(@"MacSetPrivacyMode: Failed to create CGEventTap; input blocking not enabled.");
            success = false;
        }
    };
    
    // Execute on main thread to ensure CFRunLoop operations are safe.
    // Use dispatch_sync if not on main thread, otherwise execute directly to avoid deadlock.
    //
    // IMPORTANT: Potential deadlock consideration:
    // Using dispatch_sync while holding g_privacyModeMutex could deadlock if the main thread
    // tries to acquire g_privacyModeMutex. Currently this is safe because:
    // 1. MacSetPrivacyMode (which holds the mutex) is only called from background threads
    // 2. The main thread never directly calls MacSetPrivacyMode
    // If this assumption changes in the future, consider releasing the mutex before dispatch_sync
    // or restructuring the locking strategy.
    if ([NSThread isMainThread]) {
        setupBlock();
    } else {
        dispatch_sync(dispatch_get_main_queue(), setupBlock);
    }
    
    return success;
}

// Helper function to tear down EventTap on the main thread
static void TeardownEventTapOnMainThread() {
    void (^teardownBlock)(void) = ^{
        if (g_eventTap) {
            CGEventTapEnable(g_eventTap, false);
            CFRunLoopRemoveSource(CFRunLoopGetMain(), g_runLoopSource, kCFRunLoopCommonModes);
            CFRelease(g_runLoopSource);
            CFRelease(g_eventTap);
            g_eventTap = NULL;
            g_runLoopSource = NULL;
        }
    };
    
    // Execute on main thread to ensure CFRunLoop operations are safe.
    //
    // NOTE: We use dispatch_sync here instead of dispatch_async because:
    // 1. TurnOffPrivacyModeInternal() expects EventTap to be fully torn down before
    //    proceeding with gamma restoration - using async would cause race conditions.
    // 2. The caller (MacSetPrivacyMode) needs deterministic cleanup order.
    //
    // IMPORTANT: Potential deadlock consideration (same as SetupEventTapOnMainThread):
    // Using dispatch_sync while holding g_privacyModeMutex could deadlock if the main thread
    // tries to acquire g_privacyModeMutex. Currently this is safe because:
    // 1. MacSetPrivacyMode (which holds the mutex) is only called from background threads
    // 2. The main thread never directly calls MacSetPrivacyMode
    // If this assumption changes in the future, consider releasing the mutex before dispatch_sync
    // or restructuring the locking strategy.
    if ([NSThread isMainThread]) {
        teardownBlock();
    } else {
        dispatch_sync(dispatch_get_main_queue(), teardownBlock);
    }
}

// Internal function to turn off privacy mode without acquiring the mutex
// Must be called while holding g_privacyModeMutex
static bool TurnOffPrivacyModeInternal() {
    if (!g_privacyModeActive) {
        return true;
    }
    
    // 1. Unregister display reconfiguration callback
    CGDisplayRemoveReconfigurationCallback(DisplayReconfigurationCallback, NULL);
    
    // 2. Input - restore (tear down EventTap on main thread)
    TeardownEventTapOnMainThread();

    // 3. Gamma - restore using UUID to find current DisplayID
    bool restoreSuccess = RestoreAllGammas();
    
    // 4. Fallback: Always call CGDisplayRestoreColorSyncSettings as a safety net
    // This ensures displays return to normal even if our restoration failed or
    // if the system (ColorSync/Night Shift) modified gamma during privacy mode
    CGDisplayRestoreColorSyncSettings();
    
    // Clean up
    g_originalGammas.clear();
    g_privacyModeActive = false;
    g_privacyModeShutdownRequested = false;
    g_lastReconfigTimestamp = 0;
    g_blackoutReapplicationScheduled = false;
    
    return restoreSuccess;
}

extern "C" bool MacSetPrivacyMode(bool on) {
    std::lock_guard<std::mutex> lock(g_privacyModeMutex);
    if (on) {
        // Already in privacy mode
        if (g_privacyModeActive) {
            return true;
        }
        
        // 1. Input Blocking - set up EventTap on main thread
        if (!SetupEventTapOnMainThread()) {
            return false;
        }

        // 2. Register display reconfiguration callback to handle hot-plug events
        CGDisplayRegisterReconfigurationCallback(DisplayReconfigurationCallback, NULL);

        // 3. Gamma Blackout
        uint32_t count = 0;
        CGGetOnlineDisplayList(0, NULL, &count);
        std::vector<CGDirectDisplayID> displays(count);
        CGGetOnlineDisplayList(count, displays.data(), &count);

        uint32_t blackoutSuccessCount = 0;
        uint32_t blackoutAttemptCount = 0;

        for (uint32_t i = 0; i < count; i++) {
            CGDirectDisplayID d = displays[i];
            std::string uuid = GetDisplayUUID(d);
            
            if (uuid.empty()) {
                NSLog(@"MacSetPrivacyMode: Failed to get UUID for display %u, privacy mode requires all displays", (unsigned)d);
                // Privacy mode requires ALL connected displays to be successfully blacked out 
                // to ensure user privacy. If we can't identify a display (no UUID), 
                // we can't safely manage its state or restore it later.
                // Therefore, we must abort the entire operation and clean up any resources
                // already allocated (like event taps and reconfiguration callbacks).
                CGDisplayRemoveReconfigurationCallback(DisplayReconfigurationCallback, NULL);
                TeardownEventTapOnMainThread();
                // Restore gamma for displays that were already blacked out before this failure
                if (!RestoreAllGammas()) {
                    // If any display failed to restore, use system reset as fallback
                    CGDisplayRestoreColorSyncSettings();
                }
                g_originalGammas.clear();
                return false;
            }
            
            // Save original gamma using UUID as key (stable across reconnections)
            if (g_originalGammas.find(uuid) == g_originalGammas.end()) {
                uint32_t capacity = CGDisplayGammaTableCapacity(d);
                if (capacity > 0) {
                    std::vector<CGGammaValue> red(capacity), green(capacity), blue(capacity);
                    uint32_t sampleCount = 0;
                    if (CGGetDisplayTransferByTable(d, capacity, red.data(), green.data(), blue.data(), &sampleCount) == kCGErrorSuccess) {
                        std::vector<CGGammaValue> all;
                        all.insert(all.end(), red.begin(), red.begin() + sampleCount);
                        all.insert(all.end(), green.begin(), green.begin() + sampleCount);
                        all.insert(all.end(), blue.begin(), blue.begin() + sampleCount);
                        g_originalGammas[uuid] = all;
                    } else {
                        NSLog(@"MacSetPrivacyMode: Failed to get gamma table for display %u (UUID: %s)", (unsigned)d, uuid.c_str());
                    }
                } else {
                    NSLog(@"MacSetPrivacyMode: Display %u (UUID: %s) has zero gamma table capacity, not supported", (unsigned)d, uuid.c_str());
                }
            }

            // Set to black only if we have saved original gamma for this display
            if (g_originalGammas.find(uuid) != g_originalGammas.end()) {
                uint32_t capacity = CGDisplayGammaTableCapacity(d);
                if (capacity > 0) {
                    std::vector<CGGammaValue> zeros(capacity, 0.0f);
                    blackoutAttemptCount++;
                    CGError error = CGSetDisplayTransferByTable(d, capacity, zeros.data(), zeros.data(), zeros.data());
                    if (error != kCGErrorSuccess) {
                        NSLog(@"MacSetPrivacyMode: Failed to blackout display (ID: %u, UUID: %s, error: %d)", (unsigned)d, uuid.c_str(), error);
                    } else {
                        blackoutSuccessCount++;
                    }
                } else {
                    NSLog(@"MacSetPrivacyMode: Display %u (UUID: %s) has zero gamma table capacity for blackout", (unsigned)d, uuid.c_str());
                }
            }
        }
        
        // Return false if any display failed to blackout - privacy mode requires ALL displays to be blacked out
        if (blackoutAttemptCount > 0 && blackoutSuccessCount < blackoutAttemptCount) {
            NSLog(@"MacSetPrivacyMode: Failed to blackout all displays (%u/%u succeeded)", blackoutSuccessCount, blackoutAttemptCount);
            // Clean up: unregister callback and disable event tap since we're failing
            CGDisplayRemoveReconfigurationCallback(DisplayReconfigurationCallback, NULL);
            TeardownEventTapOnMainThread();
            // Restore gamma for displays that were successfully blacked out
            if (!RestoreAllGammas()) {
                // If any display failed to restore, use system reset as fallback
                NSLog(@"Some displays failed to restore gamma during cleanup, using CGDisplayRestoreColorSyncSettings as fallback");
                CGDisplayRestoreColorSyncSettings();
            }
            g_originalGammas.clear();
            return false;
        }
        
        g_privacyModeActive = true;
        return true;

    } else {
        return TurnOffPrivacyModeInternal();
    }
}
