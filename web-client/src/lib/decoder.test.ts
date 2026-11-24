import test from 'node:test';
import assert from 'node:assert/strict';
import {
  KEYFRAME_WAIT_REFRESH_MS,
  MAX_DECODE_QUEUE_SIZE,
  REFRESH_COOLDOWN_MS,
  nextMonotonicTimestampUs,
  shouldRequestKeyframeRefresh,
} from './decoder';

test('nextMonotonicTimestampUs guarantees strictly increasing microsecond timestamps even on duplicate timer ticks', () => {
  const t1 = nextMonotonicTimestampUs(0, 100.5);
  assert.equal(t1, 100500);
  // Same coarsened performance.now() tick must advance by +1µs
  const t2 = nextMonotonicTimestampUs(t1, 100.5);
  assert.equal(t2, 100501);
  // Earlier timestamp due to jitter still advances monotonically
  const t3 = nextMonotonicTimestampUs(t2, 100.4);
  assert.equal(t3, 100502);
  // Later timestamp jumps forward normally
  const t4 = nextMonotonicTimestampUs(t3, 133.8);
  assert.equal(t4, 133800);
});

test('shouldRequestKeyframeRefresh triggers on keyframe stall or deep decode queue and respects cooldown', () => {
  // Normal healthy state -> no refresh
  assert.equal(
    shouldRequestKeyframeRefresh({
      waitingForKey: false,
      waitingSinceMs: null,
      decodeQueueSize: 2,
      lastRefreshMs: 0,
      nowMs: 5000,
    }),
    false,
  );

  // Waiting for keyframe past threshold -> refresh
  assert.equal(
    shouldRequestKeyframeRefresh({
      waitingForKey: true,
      waitingSinceMs: 3000,
      decodeQueueSize: 0,
      lastRefreshMs: 0,
      nowMs: 3000 + KEYFRAME_WAIT_REFRESH_MS,
    }),
    true,
  );

  // Cooldown suppresses duplicate rapid refreshes
  assert.equal(
    shouldRequestKeyframeRefresh({
      waitingForKey: true,
      waitingSinceMs: 3000,
      decodeQueueSize: 0,
      lastRefreshMs: 4000,
      nowMs: 4000 + REFRESH_COOLDOWN_MS - 100,
    }),
    false,
  );

  // Overloaded decode queue triggers refresh
  assert.equal(
    shouldRequestKeyframeRefresh({
      waitingForKey: false,
      waitingSinceMs: null,
      decodeQueueSize: MAX_DECODE_QUEUE_SIZE + 1,
      lastRefreshMs: 0,
      nowMs: 5000,
    }),
    true,
  );
});
