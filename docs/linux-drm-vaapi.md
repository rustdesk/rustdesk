# Experimental GPU-resident DRM capture on Linux

Build with `--features drm-vaapi` and explicitly opt in with
`RUSTDESK_DRM_VAAPI=1` in the server/service environment. The feature is off by
default. It requires the matching `hwcodec` Linux DMA-BUF input implementation.

A single-GPU unattended Wayland session using H.264 VAAPI can forward owned
scanout DMA-BUFs from the existing privileged exporter to the unprivileged
encoder. GLES3 shaders convert packed SDR RGB into limited-range BT.601 NV12
directly in VAAPI encoder surfaces. GPU writes complete before submission to
the encoder. The normal hot path performs no full-frame CPU pixel readback,
comparison, RGB-to-YUV conversion or upload. The IPC protocol and root producer
are unchanged; graphics drivers are initialized only in the user process.

The first version supports unrotated, even-sized packed 8-bit/10-bit SDR RGB
on one render device. HDR, rotated outputs, unsupported formats, ambiguous
GPU configurations and other codecs retain existing CPU-visible capture.
An import, conversion or encode failure disables the experiment for that
server process, reads back the current frame, and continues with ordinary
CPU capture plus VAAPI encoding. Reconnect/restart can retry the experiment.
Screenshots explicitly read back only the frame requested.

GPU idle detection compares every quantized 8-bit RGB pixel with an owned GPU
snapshot. Only an occlusion-query bit crosses to the CPU. During motion the
comparison runs at most every 250 ms and intervening frames are forwarded. The
first equal comparison after those intervening frames is also forwarded, so a
final reversion to an older snapshot cannot leave the viewer showing stale motion.
Once quiet, every captured frame is checked; unchanged output has three bounded
one-second repeats and then stops encoding. A new viewer joining an idle stream
rebuilds it so the viewer receives an initial frame. Import/comparison failure
rebuilds the existing CPU-visible capture path.

`RUSTDESK_DRM_GPU_IDLE=0` disables only this idle detector for matched A/B checks.
It is forwarded with the explicit GPU opt-in to the unprivileged server. The
feature remains experimental: assess GPU load, traffic, static quality and
latency. Other GPU vendors and multiple GPUs have not been validated.

A separate cursor-readback optimization is proposed in
[libdrmtap #72](https://github.com/fxd0h/libdrmtap/pull/72). It does not cache cursor
pixels and is independent of GPU encoder eligibility.

## Validation

- `cargo test --locked -p scrap --features drm-vaapi,linux-pkg-config --lib`
- Run the `hwcodec` real-hardware `linux_dmabuf` test for H.264 encode/decode,
  colour, orientation, changing pixels and subsequent RAM input.
- On the same workstation, compare two active 3840x2160 streams at the same
  FPS, H.264 quality and content. Measure server and service CPU separately,
  ensure there is no CPU quota, and verify the GPU path actually encoded frames.
- Check screenshot, reconnect and fallback behaviour. Keep a known working
  bundle and an automatic rollback timer while testing remote access.
