# Video stage diagnostics

The `video-diagnostics` Cargo feature adds per-stream timing summaries for native
video capture, encoding, sending, decoding and UI callback submission. It is
disabled by default: the added timers, counters and log calls are compiled out of
ordinary builds. It does not change codec selection, frame pacing or QoS policy.

Build both endpoints with their usual features plus `video-diagnostics`, for
example:

```sh
cargo build --locked --release --lib --features flutter,hwcodec,vram,video-diagnostics
```

Enable the relevant debug log targets **before starting the processes**:

```text
RUST_LOG=info,librustdesk::client=debug,librustdesk::server::video_service=debug,librustdesk::server::connection=debug
```

An already running GUI, service or capture process does not inherit environment
variables set later in a terminal. Use the existing platform-specific service
launch environment when diagnosing an installed service.

Each active stream emits at most one summary per second at each instrumented
site. Sparse streams report the actual elapsed window. No separate reporting
thread or timer runs while a stream is idle. Storage is constant per stream;
there are no per-frame log records or sample vectors.

| Log prefix | Timing boundaries and rate |
| --- | --- |
| `video_encode` | Successful valid-frame `capturer.frame()` calls and conversion through encoding and local dispatch. `fresh_sent_fps` counts fresh frames forwarded to at least one connection. Failed/no-change capture attempts and static refresh frames are excluded. |
| `video_send` | Time queued before the video sender and time spent in the stream send API. `messages_fps` counts successfully sent video messages, which need not equal displayed frames. |
| `video_decode` | Successful frame handling and the UI callback call. `callback_fps` counts callbacks, not physical monitor presentation. The callback can enqueue rendering work that finishes later. |

Summaries contain sample count, elapsed window, rate, average and maximum stage
times. They do not log screen pixels, input events or authentication data.
The send API duration is not a network acknowledgement or an end-to-end latency
measurement. Measurements in different processes do not establish a shared
clock. Instrumented builds have measurement overhead even when debug logs are
filtered out; use an ordinary build for final performance acceptance.

For codec comparisons, keep resolution, quality, power state, scene and number
of viewers fixed. Reconnect after changing the codec, discard warm-up, and repeat
runs in both orders. A faster H.264 run than H.265 on one GPU is a codec/backend
observation, not evidence that this instrumentation increases frame rate.
