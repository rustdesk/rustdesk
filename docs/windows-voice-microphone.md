# Android microphone into a Windows application

Android already sends microphone audio during an accepted RustDesk voice call.
By default Windows plays it on the default speakers. That does not make it an
input device for another application.

The optional Windows host setting `voice-call-output-device` selects an exact
playback-device name for incoming, accepted voice calls. For example, with a
separately installed virtual audio cable, use an administrator terminal with an
installed RustDesk service (the existing `--option` command requires both):

```
rustdesk.exe --option voice-call-output-device "CABLE Input (VB-Audio Virtual Cable)"
```

Select that cable's capture endpoint (`CABLE Output`) as the microphone in the
receiving Windows application. Begin a normal Android voice call and accept it
on the host. Existing authentication, audio permission and call acceptance still
apply. This feature installs no driver and grants no new remote permissions.

An empty option retains default speaker playback. Regular remote-desktop audio
playback is unchanged. A missing or duplicate exact name fails instead of routing
private microphone audio to an unexpected speaker. Format changes and playback
recovery retain the chosen route; closing the call drops its routed sender.
The setting is read when a voice audio thread starts; reconnect the call after
changing it. Windows only; Android's existing capture path needs no changes.

Headphones or a virtual cable without speaker monitoring prevent feedback.
Virtual-cable setup and device selection remain explicit local configuration.
