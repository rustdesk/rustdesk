import assert from 'node:assert/strict';
import test from 'node:test';
import {
  isBrowserNavOrAuxButton,
  shouldIsolateKeyboardEvent,
  shouldPreventStageTouch,
  shouldPreventWindowWheel,
} from './inputIsolation';

test('isolates all keyboard events only during active live remote control outside chrome', () => {
  assert.equal(
    shouldIsolateKeyboardEvent({ phase: 'live', chromeOpen: false, insideChromeInput: false }),
    true,
  );
  assert.equal(
    shouldIsolateKeyboardEvent({ phase: 'live', chromeOpen: true, insideChromeInput: false }),
    false,
  );
  assert.equal(
    shouldIsolateKeyboardEvent({ phase: 'live', chromeOpen: false, insideChromeInput: true }),
    false,
  );
  assert.equal(
    shouldIsolateKeyboardEvent({ phase: 'idle', chromeOpen: false, insideChromeInput: false }),
    false,
  );
  assert.equal(
    shouldIsolateKeyboardEvent({ phase: 'connecting', chromeOpen: false, insideChromeInput: false }),
    false,
  );
});

test('flags middle and browser back/forward mouse buttons for default suppression', () => {
  assert.equal(isBrowserNavOrAuxButton(0), false);
  assert.equal(isBrowserNavOrAuxButton(2), false);
  assert.equal(isBrowserNavOrAuxButton(1), true);
  assert.equal(isBrowserNavOrAuxButton(3), true);
  assert.equal(isBrowserNavOrAuxButton(4), true);
});

test('blocks window wheel during remote control unless scrolling inside chrome', () => {
  assert.equal(shouldPreventWindowWheel(true, false), true);
  assert.equal(shouldPreventWindowWheel(true, true), false);
  assert.equal(shouldPreventWindowWheel(false, false), false);
});

test('prevents native stage touch scrolling/zooming only on stage surface while live', () => {
  assert.equal(shouldPreventStageTouch(true, true), true);
  assert.equal(shouldPreventStageTouch(true, false), false);
  assert.equal(shouldPreventStageTouch(false, true), false);
});
