#import <Foundation/Foundation.h>
#import <ReplayKit/ReplayKit.h>
#import <UIKit/UIKit.h>
#import "ScreenCapture.h"

@interface ScreenCaptureHandler : NSObject <RPScreenRecorderDelegate>
@property (nonatomic, strong) RPScreenRecorder *screenRecorder;
@property (nonatomic, assign) BOOL isCapturing;
@property (nonatomic, strong) NSMutableData *frameBuffer;
@property (nonatomic, assign) CGSize lastFrameSize;
@property (nonatomic, strong) dispatch_queue_t processingQueue;
@property (nonatomic, assign) frame_callback_t frameCallback;
@property (nonatomic, assign) CFMessagePortRef localPort;
@property (nonatomic, assign) BOOL isBroadcasting;
@property (nonatomic, assign) BOOL enableMicAudio;
@property (nonatomic, assign) BOOL enableAppAudio;
@property (nonatomic, assign) audio_callback_t audioCallback;
@property (nonatomic, assign) UIInterfaceOrientation lastOrientation;
@end

@implementation ScreenCaptureHandler

static ScreenCaptureHandler *sharedHandler = nil;

+ (instancetype)sharedInstance {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        sharedHandler = [[ScreenCaptureHandler alloc] init];
    });
    return sharedHandler;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _screenRecorder = [RPScreenRecorder sharedRecorder];
        _screenRecorder.delegate = self;
        _isCapturing = NO;
        _frameBuffer = [NSMutableData dataWithCapacity:1920 * 1080 * 4]; // Initial capacity
        _lastFrameSize = CGSizeZero;
        _processingQueue = dispatch_queue_create("com.rustdesk.screencapture", DISPATCH_QUEUE_SERIAL);
        _isBroadcasting = NO;
        _lastOrientation = UIInterfaceOrientationUnknown;
        
        // Default audio settings - microphone OFF for privacy
        _enableMicAudio = NO;
        _enableAppAudio = NO;  // App audio only captures RustDesk's own audio, not useful
        
        [self setupMessagePort];
        
        // Register for orientation change notifications
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(orientationDidChange:)
                                                     name:UIDeviceOrientationDidChangeNotification
                                                   object:nil];
    }
    return self;
}

- (void)setupMessagePort {
    NSString *portName = @"com.rustdesk.screencast.port";
    
    CFMessagePortContext context = {0, (__bridge void *)self, NULL, NULL, NULL};
    Boolean shouldFreeInfo = false;
    self.localPort = CFMessagePortCreateLocal(kCFAllocatorDefault,
                                              (__bridge CFStringRef)portName,
                                              messagePortCallback,
                                              &context,
                                              &shouldFreeInfo);
    
    if (self.localPort) {
        CFRunLoopSourceRef runLoopSource = CFMessagePortCreateRunLoopSource(kCFAllocatorDefault, self.localPort, 0);
        if (runLoopSource) {
            CFRunLoopAddSource(CFRunLoopGetMain(), runLoopSource, kCFRunLoopCommonModes);
            CFRelease(runLoopSource);
        }
    }
}

- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    
    if (self.localPort) {
        CFMessagePortInvalidate(self.localPort);
        CFRelease(self.localPort);
        self.localPort = NULL;
    }
}

- (void)orientationDidChange:(NSNotification *)notification {
    UIInterfaceOrientation currentOrientation = [[UIApplication sharedApplication] statusBarOrientation];
    if (currentOrientation != self.lastOrientation) {
        self.lastOrientation = currentOrientation;
        NSLog(@"Orientation changed to: %ld", (long)currentOrientation);
        // The next frame capture will automatically pick up the new dimensions
    }
}

static CFDataRef messagePortCallback(CFMessagePortRef local, SInt32 msgid, CFDataRef data, void *info) {
    ScreenCaptureHandler *handler = (__bridge ScreenCaptureHandler *)info;
    
    if (msgid == 1 && data) {
        // Frame header
        struct FrameHeader {
            uint32_t width;
            uint32_t height;
            uint32_t dataSize;
        } header;
        
        CFDataGetBytes(data, CFRangeMake(0, sizeof(header)), (UInt8 *)&header);
        handler.lastFrameSize = CGSizeMake(header.width, header.height);
        
    } else if (msgid == 2 && data) {
        // Frame data
        dispatch_async(handler.processingQueue, ^{
            @synchronized(handler.frameBuffer) {
                [handler.frameBuffer setData:(__bridge NSData *)data];
                handler.isBroadcasting = YES;
                
                // Call callback if set
                if (handler.frameCallback) {
                    handler.frameCallback((const uint8_t *)handler.frameBuffer.bytes,
                                          (uint32_t)handler.frameBuffer.length,
                                          (uint32_t)handler.lastFrameSize.width,
                                          (uint32_t)handler.lastFrameSize.height);
                }
            }
        });
    }
    
    return NULL;
}

- (BOOL)startCapture {
    if (self.isCapturing || ![self.screenRecorder isAvailable]) {
        return NO;
    }
    
    // Configure audio based on user setting
    // This must be set before starting capture and cannot be changed during capture
    // To change microphone setting, must stop and restart capture
    self.screenRecorder.microphoneEnabled = self.enableMicAudio;
    
    __weak typeof(self) weakSelf = self;
    
    [self.screenRecorder startCaptureWithHandler:^(CMSampleBufferRef sampleBuffer, RPSampleBufferType bufferType, NSError *error) {
        if (error) {
            NSLog(@"Screen capture error: %@", error.localizedDescription);
            return;
        }
        
        switch (bufferType) {
            case RPSampleBufferTypeVideo:
                [weakSelf processSampleBuffer:sampleBuffer];
                break;
                
            case RPSampleBufferTypeAudioApp:
                // App audio only captures RustDesk's own audio, not useful
                // iOS doesn't allow capturing other apps' audio
                break;
                
            case RPSampleBufferTypeAudioMic:
                if (weakSelf.enableMicAudio && weakSelf.audioCallback) {
                    [weakSelf processAudioSampleBuffer:sampleBuffer isMic:YES];
                }
                break;
                
            default:
                break;
        }
    } completionHandler:^(NSError *error) {
        if (error) {
            NSLog(@"Failed to start capture: %@", error.localizedDescription);
            weakSelf.isCapturing = NO;
        } else {
            weakSelf.isCapturing = YES;
        }
    }];
    
    return YES;
}

- (void)stopCapture {
    if (!self.isCapturing) {
        return;
    }
    
    __weak typeof(self) weakSelf = self;
    [self.screenRecorder stopCaptureWithHandler:^(NSError *error) {
        if (error) {
            NSLog(@"Error stopping capture: %@", error.localizedDescription);
        }
        weakSelf.isCapturing = NO;
    }];
}

- (void)processSampleBuffer:(CMSampleBufferRef)sampleBuffer {
    CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (!imageBuffer) {
        return;
    }
    
    dispatch_async(self.processingQueue, ^{
        CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
        
        size_t width = CVPixelBufferGetWidth(imageBuffer);
        size_t height = CVPixelBufferGetHeight(imageBuffer);
        size_t bytesPerRow = CVPixelBufferGetBytesPerRow(imageBuffer);
        void *baseAddress = CVPixelBufferGetBaseAddress(imageBuffer);
        
        self.lastFrameSize = CGSizeMake(width, height);
        
        // Ensure buffer is large enough
        size_t requiredSize = width * height * 4;
        @synchronized(self.frameBuffer) {
            if (self.frameBuffer.length < requiredSize) {
                [self.frameBuffer setLength:requiredSize];
            }
        }
        
        @synchronized(self.frameBuffer) {
            uint8_t *src = (uint8_t *)baseAddress;
            uint8_t *dst = (uint8_t *)self.frameBuffer.mutableBytes;
            
            // Convert BGRA to RGBA
            OSType pixelFormat = CVPixelBufferGetPixelFormatType(imageBuffer);
            if (pixelFormat == kCVPixelFormatType_32BGRA) {
                for (size_t y = 0; y < height; y++) {
                    for (size_t x = 0; x < width; x++) {
                        size_t srcIdx = y * bytesPerRow + x * 4;
                        size_t dstIdx = y * width * 4 + x * 4;
                        
                        // Bounds check
                        if (srcIdx + 3 < bytesPerRow * height && dstIdx + 3 < requiredSize) {
                            dst[dstIdx + 0] = src[srcIdx + 2]; // R
                            dst[dstIdx + 1] = src[srcIdx + 1]; // G
                            dst[dstIdx + 2] = src[srcIdx + 0]; // B
                            dst[dstIdx + 3] = src[srcIdx + 3]; // A
                        }
                    }
                }
            } else {
                // Copy as-is if already RGBA
                memcpy(dst, src, MIN(requiredSize, bytesPerRow * height));
            }
            
            CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
            
            // Call the callback if set
            if (self.frameCallback) {
                self.frameCallback(dst, (uint32_t)requiredSize, (uint32_t)width, (uint32_t)height);
            }
        }
    });
}

- (NSData *)getCurrentFrame {
    @synchronized(self.frameBuffer) {
        return [self.frameBuffer copy];
    }
}

- (void)processAudioSampleBuffer:(CMSampleBufferRef)sampleBuffer isMic:(BOOL)isMic {
    // Get audio format information
    CMFormatDescriptionRef formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer);
    const AudioStreamBasicDescription *asbd = CMAudioFormatDescriptionGetStreamBasicDescription(formatDesc);
    
    if (!asbd) {
        NSLog(@"Failed to get audio format description");
        return;
    }
    
    // Verify it's PCM format we can handle
    if (asbd->mFormatID != kAudioFormatLinearPCM) {
        NSLog(@"Unsupported audio format: %u", asbd->mFormatID);
        return;
    }
    
    // Log format info once
    static BOOL loggedFormat = NO;
    if (!loggedFormat) {
        NSLog(@"Audio format - Sample rate: %.0f, Channels: %d, Bits per channel: %d, Format: %u, Flags: %u",
              asbd->mSampleRate, asbd->mChannelsPerFrame, asbd->mBitsPerChannel, 
              asbd->mFormatID, asbd->mFormatFlags);
        loggedFormat = YES;
    }
    
    // Get audio buffer list
    CMBlockBufferRef blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer);
    if (!blockBuffer) {
        // Try to get audio buffer list for interleaved audio
        AudioBufferList audioBufferList;
        size_t bufferListSizeNeededOut;
        OSStatus status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            &bufferListSizeNeededOut,
            &audioBufferList,
            sizeof(audioBufferList),
            NULL,
            NULL,
            kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            &blockBuffer
        );
        
        if (status != noErr || audioBufferList.mNumberBuffers == 0) {
            NSLog(@"Failed to get audio buffer list: %d", status);
            return;
        }
        
        // Process first buffer (assuming non-interleaved)
        AudioBuffer *audioBuffer = &audioBufferList.mBuffers[0];
        if (self.audioCallback && audioBuffer->mData && audioBuffer->mDataByteSize > 0) {
            self.audioCallback((const uint8_t *)audioBuffer->mData, 
                               (uint32_t)audioBuffer->mDataByteSize, isMic);
        }
        
        if (blockBuffer) {
            CFRelease(blockBuffer);
        }
        return;
    }
    
    size_t lengthAtOffset;
    size_t totalLength;
    char *dataPointer;
    
    OSStatus status = CMBlockBufferGetDataPointer(blockBuffer, 0, &lengthAtOffset, &totalLength, &dataPointer);
    if (status != kCMBlockBufferNoErr || !dataPointer) {
        return;
    }
    
    // Call the audio callback with proper format info
    if (self.audioCallback) {
        // Pass raw PCM data - the Rust side will handle conversion based on format
        self.audioCallback((const uint8_t *)dataPointer, (uint32_t)totalLength, isMic);
    }
}

#pragma mark - RPScreenRecorderDelegate

- (void)screenRecorderDidChangeAvailability:(RPScreenRecorder *)screenRecorder {
    NSLog(@"Screen recorder availability changed: %@", screenRecorder.isAvailable ? @"Available" : @"Not available");
}

- (void)screenRecorder:(RPScreenRecorder *)screenRecorder didStopRecordingWithPreviewViewController:(RPPreviewViewController *)previewViewController error:(NSError *)error {
    self.isCapturing = NO;
    if (error) {
        NSLog(@"Recording stopped with error: %@", error.localizedDescription);
    }
}

@end

// C interface implementation

void ios_capture_init(void) {
    [ScreenCaptureHandler sharedInstance];
}

bool ios_capture_start(void) {
    return [[ScreenCaptureHandler sharedInstance] startCapture];
}

void ios_capture_stop(void) {
    [[ScreenCaptureHandler sharedInstance] stopCapture];
}

bool ios_capture_is_active(void) {
    return [ScreenCaptureHandler sharedInstance].isCapturing;
}

uint32_t ios_capture_get_frame(uint8_t* buffer, uint32_t buffer_size, 
                               uint32_t* out_width, uint32_t* out_height) {
    ScreenCaptureHandler *handler = [ScreenCaptureHandler sharedInstance];
    
    @synchronized(handler.frameBuffer) {
        if (handler.frameBuffer.length == 0 || handler.lastFrameSize.width == 0) {
            return 0;
        }
        
        uint32_t width = (uint32_t)handler.lastFrameSize.width;
        uint32_t height = (uint32_t)handler.lastFrameSize.height;
        uint32_t frameSize = width * height * 4;
        
        if (buffer_size < frameSize) {
            return 0;
        }
        
        memcpy(buffer, handler.frameBuffer.bytes, frameSize);
        
        if (out_width) *out_width = width;
        if (out_height) *out_height = height;
        
        return frameSize;
    }
}

void ios_capture_get_display_info(uint32_t* width, uint32_t* height) {
    UIScreen *mainScreen = [UIScreen mainScreen];
    CGFloat scale = mainScreen.scale;
    CGSize screenSize = mainScreen.bounds.size;
    
    if (width) *width = (uint32_t)(screenSize.width * scale);
    if (height) *height = (uint32_t)(screenSize.height * scale);
}

void ios_capture_set_callback(frame_callback_t callback) {
    [ScreenCaptureHandler sharedInstance].frameCallback = callback;
}

void ios_capture_show_broadcast_picker(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (@available(iOS 12.0, *)) {
            RPSystemBroadcastPickerView *picker = [[RPSystemBroadcastPickerView alloc] init];
            picker.preferredExtension = @"com.carriez.rustdesk.BroadcastExtension";
            picker.showsMicrophoneButton = NO;
            
            // Add to current window temporarily
            UIWindow *window = UIApplication.sharedApplication.windows.firstObject;
            if (window) {
                picker.frame = CGRectMake(-100, -100, 100, 100);
                [window addSubview:picker];
                
                // Programmatically tap the button
                dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.1 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
                    for (UIView *subview in picker.subviews) {
                        if ([subview isKindOfClass:[UIButton class]]) {
                            [(UIButton *)subview sendActionsForControlEvents:UIControlEventTouchUpInside];
                            break;
                        }
                    }
                    
                    // Remove after a delay
                    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(1.0 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
                        [picker removeFromSuperview];
                    });
                });
            }
        }
    });
}

bool ios_capture_is_broadcasting(void) {
    return [ScreenCaptureHandler sharedInstance].isBroadcasting;
}

void ios_capture_set_audio_enabled(bool enable_mic, bool enable_app_audio) {
    ScreenCaptureHandler *handler = [ScreenCaptureHandler sharedInstance];
    handler.enableMicAudio = enable_mic;
    handler.enableAppAudio = enable_app_audio;
}

void ios_capture_set_audio_callback(audio_callback_t callback) {
    [ScreenCaptureHandler sharedInstance].audioCallback = callback;
}