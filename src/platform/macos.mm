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

// Use CFStringRef (UUID) as key instead of CGDirectDisplayID for stability across reconnections
// CGDirectDisplayID can change when displays are reconnected, but UUID remains stable
static std::map<std::string, std::vector<CGGammaValue>> g_originalGammas;

// The event source user data value used by enigo library for injected events.
// This allows us to distinguish remote input (which should be allowed) from local physical input.
// See: libs/enigo/src/macos/macos_impl.rs - ENIGO_INPUT_EXTRA_VALUE
static const int64_t ENIGO_INPUT_EXTRA_VALUE = 100;

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

// Helper function to get display name from DisplayID
static std::string GetDisplayName(CGDirectDisplayID displayId) {
    NSArray<NSScreen *> *screens = [NSScreen screens];
    for (NSScreen *screen in screens) {
        NSDictionary *deviceDescription = [screen deviceDescription];
        NSNumber *screenNumber = [deviceDescription objectForKey:@"NSScreenNumber"];
        CGDirectDisplayID screenDisplayID = [screenNumber unsignedIntValue];
        if (screenDisplayID == displayId) {
            // localizedName is available on macOS 10.15+
            if (@available(macOS 10.15, *)) {
                NSString *name = [screen localizedName];
                if (name) {
                    return std::string([name UTF8String]);
                }
            }
            break;
        }
    }
    return "Unknown";
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

// Display reconfiguration callback to handle display connect/disconnect events
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
            TurnOffPrivacyModeInternal();
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
                    TurnOffPrivacyModeInternal();
                    return;
                }
            } else {
                NSLog(@"DisplayReconfigurationCallback: Display %u (UUID: %s) has zero gamma table capacity, exiting privacy mode", (unsigned)display, uuid.c_str());
                TurnOffPrivacyModeInternal();
                return;
            }
        }
        
        // Apply blackout to the new display immediately
        if (!ApplyBlackoutToDisplay(display)) {
            NSLog(@"DisplayReconfigurationCallback: Failed to blackout display %u (UUID: %s), exiting privacy mode", (unsigned)display, uuid.c_str());
            TurnOffPrivacyModeInternal();
            return;
        }
        
        // Schedule a delayed re-application to handle ColorSync restoration
        // macOS may restore default gamma for ALL displays after a new display is added,
        // so we need to reapply blackout to all online displays, not just the new one
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(500 * NSEC_PER_MSEC)), dispatch_get_main_queue(), ^{
            std::lock_guard<std::mutex> innerLock(g_privacyModeMutex);
            if (g_privacyModeActive) {
                NSLog(@"Reapplying blackout to all displays after new display added");
                uint32_t onlineCount = 0;
                CGGetOnlineDisplayList(0, NULL, &onlineCount);
                std::vector<CGDirectDisplayID> onlineDisplays(onlineCount);
                CGGetOnlineDisplayList(onlineCount, onlineDisplays.data(), &onlineCount);
                
                for (uint32_t i = 0; i < onlineCount; i++) {
                    ApplyBlackoutToDisplay(onlineDisplays[i]);
                }
            }
        });
    } else if (flags & kCGDisplayRemoveFlag) {
        // A display was removed - update our mapping and reapply blackout to remaining displays
        NSLog(@"Display %u removed during privacy mode", (unsigned)display);
        std::string uuid = GetDisplayUUID(display);
        (void)uuid; // UUID retrieved for potential future use or logging
        
        // When a display is removed, macOS may reconfigure other displays and restore their gamma.
        // Schedule a delayed re-application of blackout to all remaining online displays.
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(500 * NSEC_PER_MSEC)), dispatch_get_main_queue(), ^{
            std::lock_guard<std::mutex> innerLock(g_privacyModeMutex);
            if (g_privacyModeActive) {
                NSLog(@"Reapplying blackout to all displays after display removal");
                uint32_t onlineCount = 0;
                CGGetOnlineDisplayList(0, NULL, &onlineCount);
                std::vector<CGDirectDisplayID> onlineDisplays(onlineCount);
                CGGetOnlineDisplayList(onlineCount, onlineDisplays.data(), &onlineCount);
                
                for (uint32_t i = 0; i < onlineCount; i++) {
                    ApplyBlackoutToDisplay(onlineDisplays[i]);
                }
            }
        });
    } else if (flags & kCGDisplaySetModeFlag) {
        // Display mode changed (could be ColorSync/Night Shift interference) - reapply blackout
        NSLog(@"Display %u mode changed during privacy mode, reapplying blackout", (unsigned)display);
        ApplyBlackoutToDisplay(display);
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
    
    // Execute on main thread to ensure CFRunLoop operations are safe
    // Use dispatch_sync if not on main thread, otherwise execute directly to avoid deadlock
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
    
    // Execute on main thread to ensure CFRunLoop operations are safe
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
    bool restoreSuccess = true;
    for (auto const& [uuid, gamma] : g_originalGammas) {
        // Find current DisplayID for this UUID (handles ID changes after reconnection)
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
                NSLog(@"Failed to restore gamma table for display %u (UUID: %s, error %d)", (unsigned)d, uuid.c_str(), error);
                restoreSuccess = false;
            }
        }
    }
    
    // 4. Fallback: Always call CGDisplayRestoreColorSyncSettings as a safety net
    // This ensures displays return to normal even if our restoration failed or
    // if the system (ColorSync/Night Shift) modified gamma during privacy mode
    CGDisplayRestoreColorSyncSettings();
    
    // Clean up
    g_originalGammas.clear();
    g_privacyModeActive = false;
    
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
                // Clean up and return false since privacy mode requires ALL displays to be blacked out
                CGDisplayRemoveReconfigurationCallback(DisplayReconfigurationCallback, NULL);
                TeardownEventTapOnMainThread();
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
                        std::string displayName = GetDisplayName(d);
                        NSLog(@"MacSetPrivacyMode: Failed to blackout display (Name: %s, ID: %u, UUID: %s, error: %d)", displayName.c_str(), (unsigned)d, uuid.c_str(), error);
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
            // We restore each display individually using saved gamma values rather than
            // calling CGDisplayRestoreColorSyncSettings() which would reset ALL displays
            // to system defaults, potentially leaving them in incorrect state if their
            // saved gamma values differed from defaults.
            bool cleanupRestoreFailed = false;
            for (auto const& [uuid, gamma] : g_originalGammas) {
                CGDirectDisplayID d = FindDisplayIdByUUID(uuid);
                if (d != kCGNullDirectDisplay) {
                    uint32_t sampleCount = gamma.size() / 3;
                    if (sampleCount > 0) {
                        const CGGammaValue* red = gamma.data();
                        const CGGammaValue* green = red + sampleCount;
                        const CGGammaValue* blue = green + sampleCount;
                        CGError error = CGSetDisplayTransferByTable(d, sampleCount, red, green, blue);
                        if (error != kCGErrorSuccess) {
                            std::string displayName = GetDisplayName(d);
                            NSLog(@"Failed to restore gamma for display (Name: %s, ID: %u, UUID: %s, error: %d)", displayName.c_str(), (unsigned)d, uuid.c_str(), error);
                            cleanupRestoreFailed = true;
                        }
                    }
                }
            }
            // If any display failed to restore, use system reset as fallback
            if (cleanupRestoreFailed) {
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
