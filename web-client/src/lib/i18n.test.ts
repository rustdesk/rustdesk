import test from 'node:test';
import assert from 'node:assert/strict';
import {
  detectSystemLocale,
  nextLocale,
  resolveEffectiveLocale,
  resolveInitialLocale,
  resolveLocalePreference,
  statusLabel,
  t,
} from './i18n';

test('resolveInitialLocale defaults to en and restores saved zh/en values', () => {
  assert.equal(resolveInitialLocale(null), 'en');
  assert.equal(resolveInitialLocale(''), 'en');
  assert.equal(resolveInitialLocale('fr'), 'en');
  assert.equal(resolveInitialLocale('zh'), 'zh');
  assert.equal(resolveInitialLocale('en'), 'en');
});

test('resolveLocalePreference defaults to default and resolves system language accurately', () => {
  assert.equal(resolveLocalePreference(null), 'default');
  assert.equal(resolveLocalePreference(''), 'default');
  assert.equal(resolveLocalePreference('default'), 'default');
  assert.equal(resolveLocalePreference('zh'), 'zh');
  assert.equal(resolveLocalePreference('en'), 'en');

  assert.equal(detectSystemLocale('zh-CN'), 'zh');
  assert.equal(detectSystemLocale('zh-TW'), 'zh');
  assert.equal(detectSystemLocale('en-US'), 'en');
  assert.equal(detectSystemLocale(null), 'en');

  assert.equal(resolveEffectiveLocale('default', 'zh-CN'), 'zh');
  assert.equal(resolveEffectiveLocale('default', 'en-GB'), 'en');
  assert.equal(resolveEffectiveLocale('en', 'zh-CN'), 'en');
  assert.equal(resolveEffectiveLocale('zh', 'en-US'), 'zh');
});

test('nextLocale toggles cleanly between en and zh and translates core labels', () => {
  assert.equal(nextLocale('en'), 'zh');
  assert.equal(nextLocale('zh'), 'en');
  assert.equal(t('en', 'displays'), 'Displays');
  assert.equal(t('zh', 'displays'), '显示器');
  assert.equal(t('en', 'languageDefault'), 'Default');
  assert.equal(t('zh', 'languageDefault'), '默认');
  assert.equal(statusLabel('en', 'live'), 'live');
  assert.equal(statusLabel('zh', 'live'), '已连接');
});
