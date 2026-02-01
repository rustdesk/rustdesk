# Hardware Encoding Quality Improvement at Low FPS

## Problem Statement

At the same bitrate, hardware encoding currently uses less bandwidth than software encoding because the hardware encoder does not set min/max rate control or buffer size parameters. This results in poorer image quality with hardware encoding at lower FPS (e.g., 10 FPS), while the quality is quite good at higher FPS.

## Root Cause

The hwcodec library (https://github.com/rustdesk-org/hwcodec) currently does not configure the following FFmpeg AVCodecContext parameters:
- `rc_max_rate`: Maximum bitrate
- `rc_min_rate`: Minimum bitrate (for CBR mode)
- `rc_buffer_size`: Rate control buffer size

Without these parameters, the hardware encoder can reduce quality to save bandwidth at lower frame rates.

## Solution

Following Sunshine's approach (https://github.com/LizardByte/Sunshine), we set:

1. **rc_max_rate = bitrate**: Caps the maximum bitrate to prevent exceeding the target
2. **rc_min_rate = bitrate**: For CBR mode, maintains consistent quality by preventing bitrate drops
3. **rc_buffer_size = bitrate / fps**: Sets buffer to one frame worth of data, preventing quality degradation at low FPS

### Code Changes

**File: `libs/hwcodec_patched/cpp/common/util.cpp`**

In the `set_av_codec_ctx()` function:

```cpp
if (kbs > 0) {
    int64_t bitrate = kbs * 1000;
    c->bit_rate = bitrate;
    
    // Set rate control parameters to maintain quality at lower FPS
    c->rc_max_rate = bitrate;
    
    if (name.find("qsv") != std::string::npos) {
        // QSV uses VBR mode with same max rate for better quality
        c->bit_rate--; // cbr with vbr
    } else {
        // For other encoders, set min rate to match max rate for CBR
        c->rc_min_rate = bitrate;
    }
    
    // Set buffer size to one frame worth of data
    if (fps > 0) {
        c->rc_buffer_size = bitrate / fps;
    }
}
```

**File: `libs/scrap/Cargo.toml`**

Changed hwcodec dependency from git to local path to use patched version:

```toml
[dependencies.hwcodec]
path = "../hwcodec_patched"
optional = true
```

## Expected Results

With these changes:
- Hardware encoding at 10 FPS with 2 Mbps bitrate should maintain image quality similar to Sunshine
- Quality should be consistent across different FPS settings (10 FPS to 30+ FPS)
- Bitrate usage will be more consistent and predictable

## Comparison with Sunshine

Sunshine (https://github.com/LizardByte/Sunshine) implements similar rate control in `src/video.cpp`:

```cpp
auto bitrate = config.bitrate * 1000;
ctx->rc_max_rate = bitrate;
ctx->bit_rate = bitrate;

if (encoder.flags & CBR_WITH_VBR) {
    ctx->bit_rate--;
} else {
    ctx->rc_min_rate = bitrate;
}

ctx->rc_buffer_size = bitrate / config.framerate;
```

Our implementation follows the same principles but adapts to the hwcodec library's architecture.

## Testing

To test this change:

1. Build RustDesk with hardware encoding enabled
2. Set video quality to use 2 Mbps bitrate at 1920x1080
3. Connect to a remote desktop and set FPS to 10
4. Compare image quality with:
   - Previous version at 10 FPS (should be worse)
   - Same version at 30 FPS (should be similar)
   - Sunshine at 10 FPS with 2 Mbps (should be comparable)

## Files Modified

1. `libs/hwcodec_patched/cpp/common/util.cpp` - Added rate control buffer settings
2. `libs/scrap/Cargo.toml` - Updated hwcodec dependency to use local patched version
3. `.gitignore` - Added hwcodec_patched to exclude git metadata

## Patch File

A patch file for the hwcodec repository is available at `hwcodec_rate_control_fix.patch` which can be submitted as a PR to https://github.com/rustdesk-org/hwcodec.

## Future Work

Once the hwcodec repository is updated with this fix, the Cargo.toml can be reverted to use the git dependency:

```toml
[dependencies.hwcodec]
git = "https://github.com/rustdesk-org/hwcodec"
optional = true
```

This is the recommended approach after the upstream fix is merged.
