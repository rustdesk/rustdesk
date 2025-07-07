#import "SampleHandler.h"
#import <os/log.h>

@interface SampleHandler ()
@property (nonatomic, strong) dispatch_queue_t videoQueue;
@property (nonatomic, assign) CFMessagePortRef messagePort;
@property (nonatomic, assign) BOOL isConnected;
@end

@implementation SampleHandler

- (instancetype)init {
    self = [super init];
    if (self) {
        _videoQueue = dispatch_queue_create("com.rustdesk.broadcast.video", DISPATCH_QUEUE_SERIAL);
        _isConnected = NO;
    }
    return self;
}

- (void)broadcastStartedWithSetupInfo:(NSDictionary<NSString *,NSObject *> *)setupInfo {
    // Create message port to communicate with main app
    NSString *portName = @"com.rustdesk.screencast.port";
    
    self.messagePort = CFMessagePortCreateRemote(kCFAllocatorDefault, (__bridge CFStringRef)portName);
    
    if (self.messagePort) {
        self.isConnected = YES;
        os_log_info(OS_LOG_DEFAULT, "Connected to main app via message port");
    } else {
        os_log_error(OS_LOG_DEFAULT, "Failed to connect to main app");
        [self finishBroadcastWithError:[NSError errorWithDomain:@"com.rustdesk.broadcast" 
                                                           code:1 
                                                       userInfo:@{NSLocalizedDescriptionKey: @"Failed to connect to main app"}]];
    }
}

- (void)broadcastPaused {
    // Handle pause
}

- (void)broadcastResumed {
    // Handle resume
}

- (void)broadcastFinished {
    if (self.messagePort) {
        CFRelease(self.messagePort);
        self.messagePort = NULL;
    }
    self.isConnected = NO;
}

- (void)processSampleBuffer:(CMSampleBufferRef)sampleBuffer withType:(RPSampleBufferType)sampleBufferType {
    if (!self.isConnected || !self.messagePort) {
        return;
    }
    
    switch (sampleBufferType) {
        case RPSampleBufferTypeVideo:
            dispatch_async(self.videoQueue, ^{
                [self processVideoSampleBuffer:sampleBuffer];
            });
            break;
            
        case RPSampleBufferTypeAudioApp:
        case RPSampleBufferTypeAudioMic:
            // Handle audio if needed
            break;
            
        default:
            break;
    }
}

- (void)processVideoSampleBuffer:(CMSampleBufferRef)sampleBuffer {
    CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (!imageBuffer) {
        return;
    }
    
    CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
    
    size_t width = CVPixelBufferGetWidth(imageBuffer);
    size_t height = CVPixelBufferGetHeight(imageBuffer);
    size_t bytesPerRow = CVPixelBufferGetBytesPerRow(imageBuffer);
    void *baseAddress = CVPixelBufferGetBaseAddress(imageBuffer);
    
    if (baseAddress) {
        // Create a header with frame info
        struct FrameHeader {
            uint32_t width;
            uint32_t height;
            uint32_t dataSize;
        } header = {
            .width = (uint32_t)width,
            .height = (uint32_t)height,
            .dataSize = (uint32_t)(width * height * 4) // Always RGBA format
        };
        
        // Send header first
        CFDataRef headerData = CFDataCreate(kCFAllocatorDefault, (const UInt8 *)&header, sizeof(header));
        
        if (headerData) {
            SInt32 result = CFMessagePortSendRequest(self.messagePort, 1, headerData, 1.0, 0.0, NULL, NULL);
            CFRelease(headerData);
            
            if (result == kCFMessagePortSuccess) {
                // Send frame data
                CFDataRef frameData = CFDataCreate(kCFAllocatorDefault, (const UInt8 *)baseAddress, header.dataSize);
                if (frameData) {
                    CFMessagePortSendRequest(self.messagePort, 2, frameData, 1.0, 0.0, NULL, NULL);
                    CFRelease(frameData);
                }
            }
        }
    }
    
    CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
}

@end