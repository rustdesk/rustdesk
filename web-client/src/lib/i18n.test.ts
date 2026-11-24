import test from 'node:test';
import assert from 'node:assert/strict';
import { nextLocale, resolveInitialLocale, statusLabel, t } from './i18n';

test('resolveInitialLocale defaults to en and restores saved zh/en values', () => {
  assert.equal(resolveInitialLocale(null), 'en');
  assert.equal(resolveInitialLocale(''), 'en');
  assert.equal(resolveInitialLocale('fr'), 'en');
  assert.equal(resolveInitialLocale('zh'), 'zh');
  assert.equal(resolveInitialLocale('en'), 'en');
});

test('nextLocale toggles cleanly between en and zh and translates core labels', () => {
  assert.equal(nextLocale('en'), 'zh');
  assert.equal(nextLocale('zh'), 'en');
  assert.equal(t('en', 'displays'), 'Displays');
  assert.equal(t('zh', 'displays'), '显示器');
  assert.equal(statusLabel('en', 'live'), 'live');
  assert.equal(statusLabel('zh', 'live'), '已连接');
});
