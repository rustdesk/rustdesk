import test from 'node:test';
import assert from 'node:assert/strict';
import {
  formatPeerId,
  removeDevice,
  toggleFavoriteDevice,
  upsertRecentDevice,
} from './devices.ts';

test('formatPeerId formats numeric RustDesk IDs into 3-digit spaced groups', () => {
  assert.equal(formatPeerId('187685134'), '187 685 134');
  assert.equal(formatPeerId('187 685 134'), '187 685 134');
  assert.equal(formatPeerId('workstation-01'), 'workstation-01');
});

test('upsertRecentDevice deduplicates by target key, preserves favorite flag, and sorts newest first', () => {
  let list = upsertRecentDevice(
    [],
    {
      mode: 'id',
      peerId: '187 685 134',
      idServer: 'vm.suijf.site:21116',
      hostname: 'MacBook Pro',
      platform: 'Mac OS',
    },
    1000,
  );
  assert.equal(list.length, 1);
  assert.equal(list[0].peerId, '187685134');

  list = toggleFavoriteDevice(list, list[0].key);
  assert.equal(list[0].favorite, true);

  list = upsertRecentDevice(
    list,
    {
      mode: 'direct',
      host: '192.168.1.50',
      port: '21118',
      hostname: 'Office PC',
      platform: 'Windows',
    },
    2000,
  );
  assert.equal(list.length, 2);
  assert.equal(list[0].hostname, 'Office PC');

  // Reconnecting to the first device moves it back to the top and preserves favorite: true
  list = upsertRecentDevice(
    list,
    {
      mode: 'id',
      peerId: '187685134',
      idServer: 'vm.suijf.site:21116',
      hostname: 'MacBook Pro',
      platform: 'Mac OS',
    },
    3000,
  );
  assert.equal(list.length, 2);
  assert.equal(list[0].hostname, 'MacBook Pro');
  assert.equal(list[0].favorite, true);

  list = removeDevice(list, list[1].key);
  assert.equal(list.length, 1);
});
