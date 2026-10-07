#include <CoreAudio/CoreAudio.h>
#include <CoreFoundation/CoreFoundation.h>
#include <vector>
#include <cstring>

static AudioObjectPropertyAddress inputProperty() {
    return {kAudioHardwarePropertyDefaultInputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
}
extern "C" int rd_mic_open(AudioDeviceID *previous, AudioDeviceID *target) {
    AudioObjectPropertyAddress devices = {kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    UInt32 size = 0;
    OSStatus status = AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &devices, 0, nullptr, &size);
    if (status) return status;
    std::vector<AudioDeviceID> ids(size / sizeof(AudioDeviceID));
    status = AudioObjectGetPropertyData(kAudioObjectSystemObject, &devices, 0, nullptr, &size, ids.data());
    if (status) return status;
    *target = 0;
    for (auto id : ids) {
        AudioObjectPropertyAddress nameProperty = {kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
        CFStringRef name = nullptr;
        UInt32 nameSize = sizeof(name);
        if (AudioObjectGetPropertyData(id, &nameProperty, 0, nullptr, &nameSize, &name) == noErr && name) {
            char value[256] = {};
            bool matches = CFStringGetCString(name, value, sizeof(value), kCFStringEncodingUTF8) && std::strcmp(value, "BlackHole 2ch") == 0;
            CFRelease(name);
            if (matches) { *target = id; break; }
        }
    }
    if (!*target) return kAudioHardwareBadDeviceError;
    auto property = inputProperty();
    size = sizeof(*previous);
    status = AudioObjectGetPropertyData(kAudioObjectSystemObject, &property, 0, nullptr, &size, previous);
    if (status) return status;
    return AudioObjectSetPropertyData(kAudioObjectSystemObject, &property, 0, nullptr, sizeof(*target), target);
}
extern "C" int rd_mic_restore(AudioDeviceID previous, AudioDeviceID target) {
    auto property = inputProperty();
    AudioDeviceID current = 0;
    UInt32 size = sizeof(current);
    OSStatus status = AudioObjectGetPropertyData(kAudioObjectSystemObject, &property, 0, nullptr, &size, &current);
    if (status || current != target) return status;
    return AudioObjectSetPropertyData(kAudioObjectSystemObject, &property, 0, nullptr, sizeof(previous), &previous);
}
