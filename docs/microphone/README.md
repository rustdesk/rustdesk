# Remote microphone forwarding (experimental)

Forward the controller microphone into a virtual audio input on the controlled computer, so an application on that computer can use it as its microphone. This is separate from speaker playback and the existing voice-call UI.

## Use

Both endpoints need a client build containing this feature; hbbs/hbbr need no changes. On the controlled computer, enable **Allow forwarded microphone** in General settings. On the controller, use the microphone button next to REC, or enable **Automatically forward microphone**. Both preferences are opt-in. Android controls are in the remote toolbar/chat menu; Android sending remains unverified.

Install/configure the receiving platform's virtual input first:

- **macOS:** BlackHole 2ch. Forwarded audio goes to its output and the default system input temporarily becomes BlackHole 2ch. Do not change the default speaker output to BlackHole. Renew screen-recording and accessibility permissions when testing a differently signed app.
- **Windows:** VB-CABLE and the AudioDeviceCmdlets PowerShell module in the logged-in user's environment. Audio goes to CABLE Input, and CABLE Output becomes the recording endpoint. Both normal and communications recording defaults are restored independently. SYSTEM/pre-login operation is unverified.
- **Linux:** PulseAudio or compatible PipeWire-Pulse with `pactl`. The implementation creates a null sink and remapped source and removes its own modules after use. Runtime behavior is unverified.

Stop forwarding or disconnect to restore the previous input. If you manually select a different input during forwarding, cleanup preserves that selection. An application that already opened its microphone may need its voice/recording session restarted or its input explicitly changed.

Desktop and Android sending are implemented. iOS/web sending and mobile system-wide virtual input receiving are unsupported. A crash can bypass input restoration; manually restore the physical microphone in system settings. A laptop must remain awake; lid-closed operation is a separate power-management issue.

## Validation

See [TESTING.md](TESTING.md) for completed checks, user-reported results and outstanding acceptance tests. The feature is experimental; a successful build does not establish runtime support for every platform.
