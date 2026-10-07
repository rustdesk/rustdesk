#include <CoreAudio/CoreAudio.h>
#include <cassert>
#include <cstdio>
extern "C" int rd_mic_open(AudioDeviceID*,AudioDeviceID*);
extern "C" int rd_mic_restore(AudioDeviceID,AudioDeviceID);
static AudioDeviceID current() {
 AudioObjectPropertyAddress p={kAudioHardwarePropertyDefaultInputDevice,kAudioObjectPropertyScopeGlobal,kAudioObjectPropertyElementMain};
 AudioDeviceID id=0; UInt32 n=sizeof(id);
 assert(AudioObjectGetPropertyData(kAudioObjectSystemObject,&p,0,nullptr,&n,&id)==noErr); return id;
}
int main(){auto before=current();AudioDeviceID prev=0,target=0;int result=rd_mic_open(&prev,&target);
 if(result==0){assert(prev==before);assert(current()==target);assert(rd_mic_restore(prev,target)==0);}
 assert(current()==before);
 printf("PASS: default input preserved/restored; virtual device open status %d\n",result);
}
