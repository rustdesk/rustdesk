import test from 'node:test';
import assert from 'node:assert/strict';
import { resolveEffectiveTheme, resolveThemePreference } from './theme';

test('resolveThemePreference defaults to system and preserves valid light/dark/system values', () => {
  assert.equal(resolveThemePreference(null), 'system');
  assert.equal(resolveThemePreference(''), 'system');
  assert.equal(resolveThemePreference('invalid'), 'system');
  assert.equal(resolveThemePreference('light'), 'light');
  assert.equal(resolveThemePreference('dark'), 'dark');
  assert.equal(resolveThemePreference('system'), 'system');
});

test('resolveEffectiveTheme resolves explicit and system theme preferences accurately', () => {
  assert.equal(resolveEffectiveTheme('light', true), 'light');
  assert.equal(resolveEffectiveTheme('dark', false), 'dark');
  assert.equal(resolveEffectiveTheme('system', true), 'dark');
  assert.equal(resolveEffectiveTheme('system', false), 'light');
});
