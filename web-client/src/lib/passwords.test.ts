import test from 'node:test';
import assert from 'node:assert/strict';
import {
  getSavedPassword,
  normalizeSavedPasswords,
  removeSavedPassword,
  setSavedPassword,
} from './passwords';

test('normalizeSavedPasswords drops invalid or empty entries while preserving valid secrets', () => {
  assert.deepEqual(normalizeSavedPasswords(null), {});
  assert.deepEqual(normalizeSavedPasswords(['a', 'b']), {});
  assert.deepEqual(
    normalizeSavedPasswords({
      'id:123456789': 'pw-1',
      'direct:10.0.0.5:21118': 'pw-2',
      '': 'ignored-empty-key',
      '   ': 'ignored-blank-key',
      'id:empty': '',
      'id:num': 42,
    }),
    {
      'id:123456789': 'pw-1',
      'direct:10.0.0.5:21118': 'pw-2',
    },
  );
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
