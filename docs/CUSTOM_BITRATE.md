# Custom Bitrate Configuration

This document describes how to configure a fixed bitrate for RustDesk remote desktop streaming, bypassing the automatic bandwidth adaptation system.

## Overview

By default, RustDesk uses an adaptive bitrate (ABR) system that automatically adjusts video quality based on network conditions. However, you may want to set a fixed target bitrate for consistent quality, especially when:

- You have a stable, high-bandwidth connection
- You want predictable bandwidth usage
- You prefer consistent quality over adaptive optimization

## Configuration

### Setting a Fixed Bitrate

To set a fixed bitrate, configure the `custom-bitrate-kbps` option:

**Example: Set 5 Mbps (5000 kbps) fixed bitrate**

The configuration method depends on your platform:

#### Desktop Application
Set the option through the RustDesk configuration:
```
custom-bitrate-kbps = 5000
```

#### Command Line
You can set this option in your configuration file or through the application settings.

### How It Works

When `custom-bitrate-kbps` is set:

1. **Adaptive Bitrate Disabled**: The automatic bandwidth adaptation system is bypassed
2. **Fixed Target**: The encoder targets your specified bitrate in kbps
3. **Dynamic Ratio Calculation**: The system calculates the appropriate quality ratio to achieve the target bitrate based on your current resolution
4. **Bounds Checking**: The ratio is clamped to safe limits to prevent encoder issues

### Examples

- **5 Mbps** (high quality): `custom-bitrate-kbps = 5000`
- **10 Mbps** (very high quality): `custom-bitrate-kbps = 10000`
- **2 Mbps** (moderate quality): `custom-bitrate-kbps = 2000`
- **1 Mbps** (low quality): `custom-bitrate-kbps = 1000`

### Disabling Custom Bitrate

To return to automatic bandwidth adaptation, either:
- Remove the `custom-bitrate-kbps` option from your configuration
- Set it to an empty value

The system will automatically revert to adaptive bitrate mode.

## Technical Details

### Implementation Location

The custom bitrate feature is implemented in:
- **File**: `src/server/video_qos.rs`
- **Struct**: `VideoQoS`

### Calculation Method

The system uses the following formula to achieve the target bitrate:

```
bitrate = base_bitrate(width, height) × ratio
```

Where:
- `base_bitrate` is calculated based on resolution
- `ratio` is dynamically adjusted to hit the target bitrate

When custom bitrate is enabled:
```
ratio = custom_bitrate_kbps × current_ratio / current_bitrate
```

This ensures the encoder targets your specified bitrate regardless of resolution changes.

### Logging

When custom bitrate is enabled, you'll see a log message:
```
Custom bitrate mode enabled: XXXX kbps
```

## Compatibility

- **Supported Codecs**: VP8, VP9, AV1, H264, H265 (all supported codecs)
- **Platforms**: All platforms (Windows, macOS, Linux, Android, iOS)
- **Encoder Types**: Both software and hardware encoders

## Notes

- The actual bitrate may vary slightly due to encoder limitations and content complexity
- Higher bitrates require more bandwidth and may cause issues on slower connections
- Very low bitrates (< 500 kbps) may result in poor quality
- The system still adapts FPS based on network conditions even with fixed bitrate

## See Also

- **Adaptive Bitrate**: Can be disabled separately with `enable-abr = N`
- **Quality Settings**: Standard quality presets (Best, Balanced, Low) are overridden when using custom bitrate
- **FPS Control**: Custom FPS can be set independently of bitrate
