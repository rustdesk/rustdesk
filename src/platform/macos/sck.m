#import <Foundation/Foundation.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdatomic.h>

static void WriteSckError(char *buffer, size_t capacity, const char *message) {
    if (buffer && capacity) {
        snprintf(buffer, capacity, "%s", message);
    }
}

#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreVideo/CVPixelBufferIOSurface.h>
#import <IOSurface/IOSurface.h>

API_AVAILABLE(macos(12.3))
@interface RustDeskSckCapture : NSObject <SCStreamOutput, SCStreamDelegate> {
    dispatch_queue_t _queue;
    dispatch_semaphore_t _shutdown;
    SCStream *_stream;
    IOSurfaceRef _latest;
    NSString *_failure;
    size_t _width;
    size_t _height;
    bool _starting;
    bool _stopRequested;
    bool _ended;
    atomic_bool _closed;
    atomic_int _shutdownResult;
    void *_closingOwner;
}
- (instancetype)initWithWidth:(size_t)width height:(size_t)height;
- (void)beginWithDisplayID:(CGDirectDisplayID)displayID;
- (int)takeSurface:(void **)surface error:(char *)buffer capacity:(size_t)capacity;
- (int)closeWithOwner:(void *)owner;
@end

@implementation RustDeskSckCapture
- (instancetype)initWithWidth:(size_t)width height:(size_t)height {
    self = [super init];
    if (self) {
        _queue = dispatch_queue_create("rustdesk.sck.capture", DISPATCH_QUEUE_SERIAL);
        _shutdown = dispatch_semaphore_create(0);
        _width = width;
        _height = height;
        _starting = true;
        atomic_init(&_closed, false);
        atomic_init(&_shutdownResult, 0);
    }
    return self;
}

- (void)clearFrame {
    if (_latest) {
        IOSurfaceDecrementUseCount(_latest);
        CFRelease(_latest);
        _latest = NULL;
    }
}

- (void)fail:(NSString *)message {
    if (!_failure) {
        _failure = message ?: @"ScreenCaptureKit capture failed";
    }
    atomic_store_explicit(&_shutdownResult, -1, memory_order_release);
    [self clearFrame];
}

- (void)releaseClosingOwner {
    void *owner = _closingOwner;
    _closingOwner = NULL;
    if (owner) {
        (void)CFBridgingRelease(owner);
    }
}

- (void)finishStopped {
    if (_ended) {
        return;
    }
    _ended = true;
    _starting = false;
    [self clearFrame];
    if (_stream) {
        NSError *error = nil;
        if (![_stream removeStreamOutput:self type:SCStreamOutputTypeScreen error:&error]) {
            [self fail:error.localizedDescription ?: @"Cannot remove ScreenCaptureKit output"];
        }
        _stream = nil;
    }
    dispatch_semaphore_signal(_shutdown);
    [self releaseClosingOwner];
}

- (void)requestStop {
    if (_starting || _stopRequested || _ended) {
        return;
    }
    if (!_stream) {
        [self finishStopped];
        return;
    }
    _stopRequested = true;
    [_stream stopCaptureWithCompletionHandler:^(NSError *error) {
        dispatch_async(self->_queue, ^{
            if (self->_ended) {
                return;
            }
            if (error) {
                [self fail:error.localizedDescription];
                // A failed stop does not prove that native callbacks have ended.
                dispatch_semaphore_signal(self->_shutdown);
            } else {
                [self finishStopped];
            }
        });
    }];
}

- (void)beginWithDisplayID:(CGDirectDisplayID)displayID {
    dispatch_async(_queue, ^{
        if (atomic_load_explicit(&self->_closed, memory_order_acquire)) {
            [self finishStopped];
            return;
        }
        [SCShareableContent getShareableContentWithCompletionHandler:
            ^(SCShareableContent *content, NSError *error) {
            dispatch_async(self->_queue, ^{
                if (atomic_load_explicit(&self->_closed, memory_order_acquire)) {
                    [self finishStopped];
                    return;
                }
                if (error || !content) {
                    [self fail:error.localizedDescription ?: @"Cannot enumerate ScreenCaptureKit displays"];
                    [self finishStopped];
                    return;
                }
                SCDisplay *selected = nil;
                for (SCDisplay *display in content.displays) {
                    if (display.displayID == displayID) {
                        selected = display;
                        break;
                    }
                }
                if (!selected) {
                    [self fail:@"Requested display is absent from ScreenCaptureKit"];
                    [self finishStopped];
                    return;
                }
                SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:selected
                                                                      excludingWindows:@[]];
                SCStreamConfiguration *config = [SCStreamConfiguration new];
                config.width = self->_width;
                config.height = self->_height;
                config.pixelFormat = kCVPixelFormatType_32BGRA;
                config.minimumFrameInterval = kCMTimeZero;
                config.showsCursor = NO;
                config.queueDepth = 3;
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 130000
                if (@available(macOS 13.0, *)) {
                    config.capturesAudio = NO;
                }
#endif
                SCStream *stream = [[SCStream alloc] initWithFilter:filter configuration:config delegate:self];
                NSError *outputError = nil;
                if (!stream || ![stream addStreamOutput:self type:SCStreamOutputTypeScreen
                                      sampleHandlerQueue:self->_queue error:&outputError]) {
                    [self fail:outputError.localizedDescription ?: @"Cannot add ScreenCaptureKit output"];
                    [self finishStopped];
                    return;
                }
                self->_stream = stream;
                [stream startCaptureWithCompletionHandler:^(NSError *startError) {
                    dispatch_async(self->_queue, ^{
                        if (self->_ended) {
                            return;
                        }
                        self->_starting = false;
                        if (startError) {
                            [self fail:startError.localizedDescription];
                            [self finishStopped];
                        } else if (self->_failure || atomic_load_explicit(&self->_closed, memory_order_acquire)) {
                            [self requestStop];
                        }
                    });
                }];
            });
        }];
    });
}

- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
        ofType:(SCStreamOutputType)type {
    if (type != SCStreamOutputTypeScreen || _ended || _failure ||
        atomic_load_explicit(&_closed, memory_order_acquire)) {
        return;
    }
    if (!sampleBuffer || !CMSampleBufferIsValid(sampleBuffer) || !CMSampleBufferDataIsReady(sampleBuffer)) {
        return;
    }
    CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
    if (!attachments || CFArrayGetCount(attachments) == 0) {
        return;
    }
    id metadata = (__bridge id)CFArrayGetValueAtIndex(attachments, 0);
    if (![metadata isKindOfClass:[NSDictionary class]]) {
        return;
    }
    id status = metadata[SCStreamFrameInfoStatus];
    if (![status isKindOfClass:[NSNumber class]] || [status integerValue] != SCFrameStatusComplete) {
        return;
    }
    CVPixelBufferRef pixels = CMSampleBufferGetImageBuffer(sampleBuffer);
    IOSurfaceRef surface = pixels ? CVPixelBufferGetIOSurface(pixels) : NULL;
    size_t stride = pixels ? CVPixelBufferGetBytesPerRow(pixels) : 0;
    if (!surface || CVPixelBufferGetPixelFormatType(pixels) != kCVPixelFormatType_32BGRA ||
        CVPixelBufferIsPlanar(pixels) || CVPixelBufferGetWidth(pixels) != _width ||
        CVPixelBufferGetHeight(pixels) != _height || _width > SIZE_MAX / 4 ||
        stride < _width * 4 || !stride || _height > SIZE_MAX / stride ||
        IOSurfaceGetWidth(surface) != _width || IOSurfaceGetHeight(surface) != _height ||
        IOSurfaceGetPixelFormat(surface) != kCVPixelFormatType_32BGRA ||
        IOSurfaceGetBytesPerRowOfPlane(surface, 0) != stride ||
        IOSurfaceGetAllocSize(surface) < stride * _height ||
        !IOSurfaceGetBaseAddress(surface) || !IOSurfaceGetBaseAddressOfPlane(surface, 0)) {
        [self fail:@"ScreenCaptureKit returned an invalid BGRA surface"];
        [self requestStop];
        return;
    }
    CFRetain(surface);
    IOSurfaceIncrementUseCount(surface);
    [self clearFrame];
    _latest = surface;
}

- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
    dispatch_async(_queue, ^{
        if (!self->_ended) {
            [self fail:error.localizedDescription ?: @"ScreenCaptureKit stream stopped"];
            [self finishStopped];
        }
    });
}

- (int)takeSurface:(void **)surface error:(char *)buffer capacity:(size_t)capacity {
    __block int result = 0;
    dispatch_sync(_queue, ^{
        if (self->_failure || self->_ended || atomic_load_explicit(&self->_closed, memory_order_acquire)) {
            WriteSckError(buffer, capacity, self->_failure ? self->_failure.UTF8String : "ScreenCaptureKit stream closed");
            result = -1;
        } else if (self->_latest) {
            *surface = self->_latest;
            self->_latest = NULL;
            result = 1;
        }
    });
    return result;
}

- (int)closeWithOwner:(void *)owner {
    atomic_store_explicit(&_closed, true, memory_order_release);
    dispatch_async(_queue, ^{
        // Transfer Create's +1 to shutdown; timeout must not release a live stream.
        self->_closingOwner = owner;
        [self clearFrame];
        if (self->_ended) {
            [self releaseClosingOwner];
        } else {
            [self requestStop];
        }
    });
    if (dispatch_semaphore_wait(_shutdown, dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC))) {
        return -2;
    }
    return atomic_load_explicit(&_shutdownResult, memory_order_acquire);
}

- (void)dealloc {
    [self clearFrame];
}
@end
#endif

bool RustDeskSckSupported(void) {
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
    if (@available(macOS 12.3, *)) {
        return [SCStream class] != Nil && [SCShareableContent class] != Nil;
    }
#endif
    return false;
}

void *RustDeskSckCreate(unsigned int display_id, unsigned int width, unsigned int height) {
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
    if (@available(macOS 12.3, *)) {
        if (RustDeskSckSupported() && display_id && width && height) {
            @autoreleasepool {
                RustDeskSckCapture *capture = [[RustDeskSckCapture alloc] initWithWidth:width height:height];
                [capture beginWithDisplayID:display_id];
                return (void *)CFBridgingRetain(capture);
            }
        }
    }
#else
    (void)display_id;
    (void)width;
    (void)height;
#endif
    return NULL;
}

int RustDeskSckTakeFrame(void *handle, void **surface, char *error_buffer, size_t capacity) {
    if (surface) {
        *surface = NULL;
    }
    WriteSckError(error_buffer, capacity, "");
    if (!handle || !surface) {
        WriteSckError(error_buffer, capacity, "Invalid ScreenCaptureKit handle or output");
        return -1;
    }
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
    if (@available(macOS 12.3, *)) {
        @autoreleasepool {
            RustDeskSckCapture *capture = (__bridge RustDeskSckCapture *)handle;
            return [capture takeSurface:surface error:error_buffer capacity:capacity];
        }
    }
#endif
    WriteSckError(error_buffer, capacity, "ScreenCaptureKit is unavailable");
    return -1;
}

int RustDeskSckDestroy(void *handle) {
    if (!handle) {
        return 0;
    }
#if __has_include(<ScreenCaptureKit/ScreenCaptureKit.h>)
    if (@available(macOS 12.3, *)) {
        @autoreleasepool {
            RustDeskSckCapture *capture = (__bridge RustDeskSckCapture *)handle;
            return [capture closeWithOwner:handle];
        }
    }
#endif
    return -1;
}
