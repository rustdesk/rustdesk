#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#include <atomic>
#include <mutex>
#include <chrono>
#include <thread>

// CoreGraphics does not publish these interfaces in its SDK. Resolve classes at
// runtime so systems without the private API can still launch RustDesk.
@interface NSObject (RustDeskVirtualDisplay)
- (id)initWithDescriptor:(id)descriptor;
- (id)initWithWidth:(unsigned int)width height:(unsigned int)height refreshRate:(double)rate;
- (BOOL)applySettings:(id)settings;
@end

static std::mutex displayMutex;
static std::atomic<unsigned int> displayMask{0};
static NSMutableDictionary<NSNumber *, id> *displays;
static NSMutableDictionary<NSNumber *, id> *displaySettings;
static std::atomic<unsigned int> displayIDs[5]{};

extern "C" bool RustDeskVirtualDisplaySupported() {
    if (@available(macOS 11.0, *)) {
        return NSClassFromString(@"CGVirtualDisplay") &&
            NSClassFromString(@"CGVirtualDisplayDescriptor") &&
            NSClassFromString(@"CGVirtualDisplayMode") &&
            NSClassFromString(@"CGVirtualDisplaySettings");
    }
    return false;
}

extern "C" unsigned int RustDeskVirtualDisplayMask() {
    return displayMask.load();
}

extern "C" bool RustDeskToggleVirtualDisplay(int index, bool on) {
    std::lock_guard<std::mutex> lock(displayMutex);
    @autoreleasepool {
        @try {
            if (!on && index == -1) {
                for (int i = 1; i <= 4; ++i) displayIDs[i].store(0);
                [displays removeAllObjects];
                [displaySettings removeAllObjects];
                displayMask.store(0);
                return true;
            }
            if (index < 1 || index > 4) return false;
            if (!on) {
                displayIDs[index].store(0);
                [displays removeObjectForKey:@(index)];
                [displaySettings removeObjectForKey:@(index)];
                displayMask.fetch_and(~(1u << index));
                return true;
            }
            if (!RustDeskVirtualDisplaySupported()) return false;
            if (displays[@(index)]) return true;
            id descriptor = [[NSClassFromString(@"CGVirtualDisplayDescriptor") alloc] init];
            [descriptor setValue:[NSString stringWithFormat:@"RustDesk %d", index] forKey:@"name"];
            [descriptor setValue:dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0) forKey:@"queue"];
            [descriptor setValue:@4096 forKey:@"maxPixelsWide"];
            [descriptor setValue:@4096 forKey:@"maxPixelsHigh"];
            [descriptor setValue:[NSValue valueWithSize:NSMakeSize(508, 285.75)] forKey:@"sizeInMillimeters"];
            // Sonoma requires a nonzero vendor and a distinct serial per display.
            [descriptor setValue:@0x5255 forKey:@"vendorID"];
            [descriptor setValue:@1 forKey:@"productID"];
            [descriptor setValue:@(index) forKey:@"serialNum"];
            [descriptor setValue:@(index) forKey:@"serialNumber"];
            [descriptor setValue:[NSValue valueWithPoint:NSMakePoint(0.64, 0.33)] forKey:@"redPrimary"];
            [descriptor setValue:[NSValue valueWithPoint:NSMakePoint(0.30, 0.60)] forKey:@"greenPrimary"];
            [descriptor setValue:[NSValue valueWithPoint:NSMakePoint(0.15, 0.06)] forKey:@"bluePrimary"];
            [descriptor setValue:[NSValue valueWithPoint:NSMakePoint(0.3127, 0.3290)] forKey:@"whitePoint"];
            id display = [[NSClassFromString(@"CGVirtualDisplay") alloc] initWithDescriptor:descriptor];
            if (!display) return false;
            id mode = [[NSClassFromString(@"CGVirtualDisplayMode") alloc]
                initWithWidth:1920 height:1080 refreshRate:60.0];
            if (!mode) return false;
            id settings = [[NSClassFromString(@"CGVirtualDisplaySettings") alloc] init];
            [settings setValue:@0 forKey:@"hiDPI"];
            [settings setValue:@[mode] forKey:@"modes"];
            if (![display applySettings:settings]) return false;
            unsigned int displayID = [[display valueForKey:@"displayID"] unsignedIntValue];
            if (!displayID) return false;
            if (!displays) displays = [NSMutableDictionary new];
            displays[@(index)] = display;
            if (!displaySettings) displaySettings = [NSMutableDictionary new];
            displaySettings[@(index)] = settings;
            displayIDs[index].store(displayID);
            displayMask.fetch_or(1u << index);
            return true;
        } @catch (NSException *exception) {
            NSLog(@"RustDesk virtual display failed: %@", exception);
            return false;
        }
    }
}

extern "C" bool RustDeskOwnsVirtualDisplay(unsigned int displayID) {
    if (!displayID) return false;
    for (int i = 1; i <= 4; ++i) {
        if (displayIDs[i].load() == displayID) return true;
    }
    return false;
}

extern "C" bool RustDeskVirtualDisplayMode(unsigned int displayID, unsigned int *width, unsigned int *height, unsigned int *scale) {
    if (!width || !height || !scale || !RustDeskOwnsVirtualDisplay(displayID)) return false;
    CGDisplayModeRef mode = CGDisplayCopyDisplayMode(displayID);
    if (!mode) return false;
    auto logicalWidth = CGDisplayModeGetWidth(mode);
    auto logicalHeight = CGDisplayModeGetHeight(mode);
    auto pixelWidth = CGDisplayModeGetPixelWidth(mode);
    auto pixelHeight = CGDisplayModeGetPixelHeight(mode);
    CGDisplayModeRelease(mode);
    if (!logicalWidth || !logicalHeight || pixelWidth % logicalWidth) return false;
    auto nativeScale = pixelWidth / logicalWidth;
    if ((nativeScale != 1 && nativeScale != 2) || pixelHeight != logicalHeight * nativeScale ||
        pixelWidth < 320 || pixelWidth > 4096 || pixelHeight < 320 || pixelHeight > 4096 ||
        !RustDeskOwnsVirtualDisplay(displayID)) return false;
    *width = static_cast<unsigned int>(pixelWidth);
    *height = static_cast<unsigned int>(pixelHeight);
    *scale = static_cast<unsigned int>(nativeScale);
    return true;
}

static bool selectMode(unsigned int displayID, unsigned int width, unsigned int height, unsigned int scale) {
    if (scale != 1 && scale != 2) return false;
    NSDictionary *options = @{(__bridge NSString *)kCGDisplayShowDuplicateLowResolutionModes: @YES};
    for (int attempt = 0; attempt < 40; ++attempt) {
        CFArrayRef modes = CGDisplayCopyAllDisplayModes(displayID, (__bridge CFDictionaryRef)options);
        if (modes) {
            for (CFIndex j = 0; j < CFArrayGetCount(modes); ++j) {
                auto mode = (CGDisplayModeRef)CFArrayGetValueAtIndex(modes, j);
                if (CGDisplayModeGetWidth(mode) == width / scale &&
                    CGDisplayModeGetHeight(mode) == height / scale &&
                    CGDisplayModeGetPixelWidth(mode) == width &&
                    CGDisplayModeGetPixelHeight(mode) == height) {
                    bool applied = CGDisplaySetDisplayMode(displayID, mode, nullptr) == kCGErrorSuccess;
                    CFRelease(modes);
                    return applied;
                }
            }
            CFRelease(modes);
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    return false;
}

extern "C" bool RustDeskConfigureVirtualDisplay(unsigned int displayID, unsigned int width, unsigned int height, unsigned int scale) {
    if (scale != 1 && scale != 2) return false;
    if (width < 320 || height < 320 || width > 4096 || height > 4096 || width % scale || height % scale) return false;
    std::lock_guard<std::mutex> lock(displayMutex);
    @autoreleasepool {
        @try {
            for (int i = 1; i <= 4; ++i) {
                if (!displayID || displayIDs[i].load() != displayID) continue;
                id mode = [[NSClassFromString(@"CGVirtualDisplayMode") alloc]
                    initWithWidth:width / scale height:height / scale refreshRate:60.0];
                if (!mode) return false;
                id settings = [[NSClassFromString(@"CGVirtualDisplaySettings") alloc] init];
                [settings setValue:@(scale == 2) forKey:@"hiDPI"];
                [settings setValue:@[mode] forKey:@"modes"];
                struct ModeGuard {
                    CGDisplayModeRef value;
                    ~ModeGuard() { if (value) CGDisplayModeRelease(value); }
                } previousMode{CGDisplayCopyDisplayMode(displayID)};
                bool success = [displays[@(i)] applySettings:settings] && selectMode(displayID, width, height, scale);
                if (success) {
                    displaySettings[@(i)] = settings;
                } else {
                    if (![displays[@(i)] applySettings:displaySettings[@(i)]]) {
                        NSLog(@"RustDesk virtual display settings rollback failed");
                    } else if (previousMode.value) {
                        auto oldWidth = CGDisplayModeGetWidth(previousMode.value);
                        auto oldPixels = CGDisplayModeGetPixelWidth(previousMode.value);
                        if (!selectMode(displayID, oldPixels, CGDisplayModeGetPixelHeight(previousMode.value), oldWidth ? oldPixels / oldWidth : 1)) {
                            NSLog(@"RustDesk virtual display mode rollback failed");
                        }
                    }
                }
                return success;
            }
        } @catch (NSException *exception) {
            NSLog(@"RustDesk virtual display configuration failed: %@", exception);
        }
    }
    return false;
}

extern "C" bool RustDeskResizeVirtualDisplay(unsigned int displayID, unsigned int width, unsigned int height) {
    if (!RustDeskOwnsVirtualDisplay(displayID) || width > 4096 || height > 4096) return false;
    CGDisplayModeRef current = CGDisplayCopyDisplayMode(displayID);
    if (!current) return false;
    auto logicalWidth = CGDisplayModeGetWidth(current);
    unsigned int scale = logicalWidth ? CGDisplayModeGetPixelWidth(current) / logicalWidth : 1;
    CGDisplayModeRelease(current);
    return RustDeskConfigureVirtualDisplay(displayID, width * scale, height * scale, scale);
}
