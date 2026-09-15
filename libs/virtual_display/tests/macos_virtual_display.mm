#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <AppKit/AppKit.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdio>
#include <thread>
#include <vector>

extern "C" bool RustDeskVirtualDisplaySupported();
extern "C" bool RustDeskVirtualDisplayMode(unsigned int, unsigned int *, unsigned int *, unsigned int *);
extern "C" unsigned int RustDeskVirtualDisplayMask();
extern "C" bool RustDeskToggleVirtualDisplay(int, bool);

extern "C" bool RustDeskOwnsVirtualDisplay(unsigned int);
extern "C" bool RustDeskResizeVirtualDisplay(unsigned int, unsigned int, unsigned int);

extern "C" bool RustDeskConfigureVirtualDisplay(unsigned int, unsigned int, unsigned int, unsigned int);

static bool testResolution(unsigned int width, unsigned int height, unsigned int scale = 1) {
    CGDirectDisplayID ids[32];
    uint32_t count = 0;
    if (CGGetOnlineDisplayList(32, ids, &count) != kCGErrorSuccess) return false;
    for (uint32_t i = 0; i < count; ++i) {
        if (!RustDeskOwnsVirtualDisplay(ids[i])) continue;
        if (RustDeskResizeVirtualDisplay(ids[i], 0, height)) return false;
        if (RustDeskResizeVirtualDisplay(ids[i], 4097, height)) return false;
        if (!RustDeskConfigureVirtualDisplay(ids[i], width, height, scale)) {
            fprintf(stderr, "Configuration failed for %ux%u@%u\n", width, height, scale);
            return false;
        }
        // Legacy clients send logical dimensions and must preserve active HiDPI.
        if (!RustDeskResizeVirtualDisplay(ids[i], width / scale, height / scale)) {
            fprintf(stderr, "Logical resize failed for %ux%u@%u\n", width, height, scale);
            return false;
        }
        unsigned int actualWidth = 0, actualHeight = 0, actualScale = 0;
        if (!RustDeskVirtualDisplayMode(ids[i], &actualWidth, &actualHeight, &actualScale) ||
            actualWidth != width || actualHeight != height || actualScale != scale) {
            fprintf(stderr, "Native mode mismatch: expected %ux%u@%u, got %ux%u@%u\n",
                width, height, scale, actualWidth, actualHeight, actualScale);
            return false;
        }
        for (int attempt = 0; attempt < 100; ++attempt) {
            CGDisplayModeRef mode = CGDisplayCopyDisplayMode(ids[i]);
            if (!mode) return false;
            bool matches = CGDisplayModeGetWidth(mode) == width / scale &&
                CGDisplayModeGetHeight(mode) == height / scale &&
                CGDisplayModeGetPixelWidth(mode) == width && CGDisplayModeGetPixelHeight(mode) == height;
            CGDisplayModeRelease(mode);
            if (matches) return true;
            std::this_thread::sleep_for(std::chrono::milliseconds(50));
        }
        return false;
    }
    return false;
}

static std::vector<CGDirectDisplayID> initialDisplays;

static bool waitForDisplays(unsigned int expected) {
    for (int attempt = 0; attempt < 100; ++attempt) {
        uint32_t count = 0;
        if (CGGetOnlineDisplayList(0, nullptr, &count) != kCGErrorSuccess) return false;
        std::vector<CGDirectDisplayID> displays(count);
        if (CGGetOnlineDisplayList(count, displays.data(), &count) != kCGErrorSuccess) return false;
        unsigned int mask = 0;
        bool matches = true;
        for (uint32_t i = 0; i < count; ++i) {
            auto display = displays[i];
            if (std::find(initialDisplays.begin(), initialDisplays.end(), display) != initialDisplays.end()) continue;
            if (CGDisplayVendorNumber(display) != 0x5255 || CGDisplayModelNumber(display) != 1) continue;
            auto serial = CGDisplaySerialNumber(display);
            if (serial < 1 || serial > 4) { matches = false; break; }
            mask |= 1u << serial;
            if (expected & (1u << serial)) {
                if (CGDisplayPixelsWide(display) != 1920 || CGDisplayPixelsHigh(display) != 1080 ||
                    !CGDisplayIsActive(display)) { matches = false; break; }
            }
        }
        if (matches && mask == expected) return true;
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    fprintf(stderr, "System display list did not reach mask %u\n", expected);
    return false;
}

static bool testExternalModeChange() {
    CGDirectDisplayID ids[32], displayID = 0;
    uint32_t count = 0;
    if (CGGetOnlineDisplayList(32, ids, &count) != kCGErrorSuccess) return false;
    for (uint32_t i = 0; i < count; ++i) {
        if (RustDeskOwnsVirtualDisplay(ids[i])) { displayID = ids[i]; break; }
    }
    if (!displayID || !RustDeskConfigureVirtualDisplay(displayID, 2560, 1600, 2)) {
        fprintf(stderr, "External mode test setup failed\n");
        return false;
    }
    auto before = CGDisplayBounds(displayID);
    NSDictionary *options = @{(__bridge NSString *)kCGDisplayShowDuplicateLowResolutionModes: @YES};
    CFArrayRef modes = CGDisplayCopyAllDisplayModes(displayID, (__bridge CFDictionaryRef)options);
    bool changed = false;
    for (CFIndex i = 0; modes && i < CFArrayGetCount(modes); ++i) {
        auto mode = (CGDisplayModeRef)CFArrayGetValueAtIndex(modes, i);
        if (CGDisplayModeGetWidth(mode) == 1280 && CGDisplayModeGetHeight(mode) == 800 &&
            CGDisplayModeGetPixelWidth(mode) == 1280 && CGDisplayModeGetPixelHeight(mode) == 800 &&
            CGDisplaySetDisplayMode(displayID, mode, nullptr) == kCGErrorSuccess) {
            changed = true;
            break;
        }
    }
    if (modes) CFRelease(modes);
    if (!changed) {
        fprintf(stderr, "External low-resolution mode selection failed\n");
        return false;
    }
    unsigned int width = 0, height = 0, scale = 0;
    if (!CGRectEqualToRect(before, CGDisplayBounds(displayID)) ||
        !RustDeskVirtualDisplayMode(displayID, &width, &height, &scale) ||
        width != 1280 || height != 800 || scale != 1) {
        auto after = CGDisplayBounds(displayID);
        fprintf(stderr, "External mode mismatch: %ux%u@%u, bounds %.0f,%.0f,%.0f,%.0f -> %.0f,%.0f,%.0f,%.0f\n",
            width, height, scale, before.origin.x, before.origin.y, before.size.width, before.size.height,
            after.origin.x, after.origin.y, after.size.width, after.size.height);
        return false;
    }
    return RustDeskConfigureVirtualDisplay(displayID, 1920, 1080, 1);
}

static bool testStableDisplayIdentity() {
    CGDirectDisplayID ids[32], first = 0, second = 0;
    uint32_t count = 0;
    if (CGGetOnlineDisplayList(32, ids, &count) != kCGErrorSuccess) return false;
    for (uint32_t i = 0; i < count; ++i) {
        if (!RustDeskOwnsVirtualDisplay(ids[i])) continue;
        if (CGDisplaySerialNumber(ids[i]) == 1) first = ids[i];
        if (CGDisplaySerialNumber(ids[i]) == 2) second = ids[i];
    }
    if (!first || !second || !RustDeskToggleVirtualDisplay(1, false) || !waitForDisplays(28)) return false;
    // Removing a preceding display must neither retarget a request nor accept its stale ID.
    if (!RustDeskConfigureVirtualDisplay(second, 2560, 1600, 2)) return false;
    if (RustDeskConfigureVirtualDisplay(first, 1920, 1080, 1)) return false;
    unsigned int width = 0, height = 0, scale = 0;
    if (!RustDeskVirtualDisplayMode(second, &width, &height, &scale) ||
        width != 2560 || height != 1600 || scale != 2) return false;
    if (!RustDeskConfigureVirtualDisplay(second, 1920, 1080, 1)) return false;
    return RustDeskToggleVirtualDisplay(1, true) && waitForDisplays(30);
}

static int runTests(int argc) {
    @autoreleasepool {
        bool supported = RustDeskVirtualDisplaySupported();
        printf("API supported: %d\n", supported);
        if (RustDeskVirtualDisplayMask() != 0) return 1;
        unsigned int width = 0, height = 0, scale = 0;
        if (RustDeskVirtualDisplayMode(0, &width, &height, &scale)) return 19;
        if (RustDeskToggleVirtualDisplay(0, true)) return 2;
        if (RustDeskToggleVirtualDisplay(5, true)) return 3;
        if (RustDeskToggleVirtualDisplay(-1, true)) return 4;
        if (!RustDeskToggleVirtualDisplay(-1, false)) return 5;
        if (argc == 1) return 0;
        uint32_t initialCount = 0;
        if (CGGetOnlineDisplayList(0, nullptr, &initialCount) != kCGErrorSuccess) return 18;
        initialDisplays.resize(initialCount);
        if (CGGetOnlineDisplayList(initialCount, initialDisplays.data(), &initialCount) != kCGErrorSuccess) return 18;
        initialDisplays.resize(initialCount);
        struct Cleanup { ~Cleanup() { RustDeskToggleVirtualDisplay(-1, false); } } cleanup;
        if (!supported || !waitForDisplays(0)) return 6;
        for (int i = 1; i <= 4; ++i) {
            if (!RustDeskToggleVirtualDisplay(i, true)) return 7;
            if (!RustDeskToggleVirtualDisplay(i, true)) return 8;
        }
        if (RustDeskVirtualDisplayMask() != 30 || !waitForDisplays(30)) return 9;
        if (!testResolution(320, 320, 2) || !testResolution(2560, 1600, 2) || !testResolution(1080, 1920, 2) || !testResolution(2560, 1600) || !testResolution(1080, 1920) || !testResolution(1920, 1080)) return 16;
        if (RustDeskResizeVirtualDisplay(0, 1920, 1080)) return 17;
        if (!testResolution(2882, 1802, 2) || !testResolution(1441, 901) || !testResolution(1920, 1080)) return 22;
        if (!testExternalModeChange()) return 21;
        if (!testStableDisplayIdentity()) return 20;
        if (!RustDeskToggleVirtualDisplay(2, false)) return 10;
        if (RustDeskVirtualDisplayMask() != 26 || !waitForDisplays(26)) return 11;
        if (!RustDeskToggleVirtualDisplay(-1, false)) return 12;
        if (RustDeskVirtualDisplayMask() != 0 || !waitForDisplays(0)) return 13;
        if (!RustDeskToggleVirtualDisplay(1, true) || !waitForDisplays(2)) return 14;
        if (!RustDeskToggleVirtualDisplay(-1, false) || !waitForDisplays(0)) return 15;
        puts("System enumeration, resolution, creation, idempotency, removal and recreation passed");
    }
    return 0;
}

int main(int argc, char **) {
    @autoreleasepool {
        // CoreGraphics mode updates need the main AppKit loop, as in the application.
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
        std::atomic<bool> finished{false};
        int result = 1;
        std::thread worker([&] {
            result = runTests(argc);
            finished.store(true);
        });
        while (!finished.load()) {
            NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny
                untilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]
                inMode:NSDefaultRunLoopMode dequeue:YES];
            if (event) [NSApp sendEvent:event];
        }
        worker.join();
        return result;
    }
}
