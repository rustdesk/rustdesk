import test from 'node:test';
import assert from 'node:assert/strict';
import {
  getSavedPassword,
  clearLegacyPasswordStorage,
  removeSavedPassword,
  setSavedPassword,
} from './passwords';

test('clears only the old password storage entries', () => {
  const storage = new Map([
    ['rd_web_saved_passwords', '{"id:123456789":"previous-password"}'],
    ['rd_web_remember_password', '1'],
    ['rd_web_prefs', '{"peerId":"123456789"}'],
  ]);
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { localStorage: { removeItem: storage.delete.bind(storage) } },
  });
  try {
    clearLegacyPasswordStorage();
    assert.deepEqual([...storage.keys()], ['rd_web_prefs']);
  } finally {
    Reflect.deleteProperty(globalThis, 'window');
  }
});

test('setSavedPassword, getSavedPassword, and removeSavedPassword behave immutably', () => {
  const initial = {};
  const withOne = setSavedPassword(initial, 'id:999', 'secret-1');
  assert.equal(getSavedPassword(withOne, 'id:999'), 'secret-1');
  assert.equal(getSavedPassword(withOne, 'id:missing'), '');
  assert.deepEqual(initial, {});

  // Setting same value returns identical reference
  assert.equal(setSavedPassword(withOne, 'id:999', 'secret-1'), withOne);

  // Updating overwrites cleanly
  const updated = setSavedPassword(withOne, 'id:999', 'secret-2');
  assert.equal(getSavedPassword(updated, 'id:999'), 'secret-2');

  // Empty password removes the saved entry
  const clearedByEmpty = setSavedPassword(updated, 'id:999', '');
  assert.equal(getSavedPassword(clearedByEmpty, 'id:999'), '');
  assert.deepEqual(clearedByEmpty, {});

  // Removing a missing key returns same object reference
  assert.equal(removeSavedPassword(clearedByEmpty, 'id:999'), clearedByEmpty);

  // Removing an existing key drops only that device
  const multi = setSavedPassword(withOne, 'direct:127.0.0.1:21118', 'direct-pw');
  const afterRemove = removeSavedPassword(multi, 'id:999');
  assert.equal(getSavedPassword(afterRemove, 'id:999'), '');
  assert.equal(getSavedPassword(afterRemove, 'direct:127.0.0.1:21118'), 'direct-pw');
});
