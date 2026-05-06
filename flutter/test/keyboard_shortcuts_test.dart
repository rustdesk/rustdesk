import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_actions.dart';
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_constants.dart';
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_utils.dart';

ShortcutPlatformCapabilities capabilities({
  bool includeFullscreenShortcut = true,
  bool includeScreenshotShortcut = true,
  bool includeTabShortcuts = true,
  bool includeToolbarShortcut = true,
  bool includeCloseTabShortcut = true,
  bool includeSwitchSidesShortcut = true,
  bool includeRecordingShortcut = true,
  bool includeResetCanvasShortcut = true,
  bool includePinToolbarShortcut = true,
  bool includeViewModeShortcut = true,
  bool includeInputSourceShortcut = true,
  bool includeVoiceCallShortcut = true,
}) {
  return ShortcutPlatformCapabilities(
    includeFullscreenShortcut: includeFullscreenShortcut,
    includeScreenshotShortcut: includeScreenshotShortcut,
    includeTabShortcuts: includeTabShortcuts,
    includeToolbarShortcut: includeToolbarShortcut,
    includeCloseTabShortcut: includeCloseTabShortcut,
    includeSwitchSidesShortcut: includeSwitchSidesShortcut,
    includeRecordingShortcut: includeRecordingShortcut,
    includeResetCanvasShortcut: includeResetCanvasShortcut,
    includePinToolbarShortcut: includePinToolbarShortcut,
    includeViewModeShortcut: includeViewModeShortcut,
    includeInputSourceShortcut: includeInputSourceShortcut,
    includeVoiceCallShortcut: includeVoiceCallShortcut,
  );
}

void main() {
  test('kDefaultShortcutBindings matches fixture', () {
    // The fixture is the cross-language source of truth for default
    // bindings. Rust has its own parity test against the same file
    // (`default_bindings_match_fixture_json` in src/keyboard/shortcuts.rs),
    // so a drift on either side breaks CI.
    final fixturePath = 'test/fixtures/default_keyboard_shortcuts.json';
    final fixture =
        jsonDecode(File(fixturePath).readAsStringSync()) as List<dynamic>;
    expect(kDefaultShortcutBindings, equals(fixture),
        reason: 'kDefaultShortcutBindings drifted from $fixturePath — update '
            'shortcut_constants.dart, the fixture, and Rust default_bindings() '
            'together');
  });

  test('save order preserves macOS control modifier', () {
    expect(canonicalShortcutModsForSave({'ctrl'}), ['ctrl']);
    expect(canonicalShortcutModsForSave({'shift', 'ctrl', 'primary', 'alt'}),
        ['primary', 'ctrl', 'alt', 'shift']);
  });

  test('shortcutBindingMapsFrom ignores malformed bindings', () {
    expect(shortcutBindingMapsFrom('not a list'), isEmpty);

    final bindings = shortcutBindingMapsFrom([
      {
        'action': kShortcutActionScreenshot,
        'mods': ['primary'],
        'key': 'p',
      },
      'bad',
      1,
      {
        'action': kShortcutActionToggleMute,
        'mods': ['alt'],
        'key': 's',
      },
    ]);

    expect(bindings, hasLength(2));
    expect(bindings.map((binding) => binding['action']), [
      kShortcutActionScreenshot,
      kShortcutActionToggleMute,
    ]);
  });

  test('shortcutModSetFrom ignores malformed modifiers', () {
    expect(shortcutModSetFrom('not a list'), isEmpty);
    expect(shortcutModSetFrom(['primary', 1, 'alt', null, 'primary']), {
      'primary',
      'alt',
    });
  });

  test('non-desktop defaults exclude desktop-only and tab shortcuts', () {
    final defaults = [
      {
        'action': kShortcutActionSendCtrlAltDel,
        'mods': ['primary', 'alt', 'shift'],
        'key': 'delete',
      },
      {
        'action': kShortcutActionToggleFullscreen,
        'mods': ['primary', 'alt', 'shift'],
        'key': 'enter',
      },
      {
        'action': kShortcutActionSwitchDisplayNext,
        'mods': ['primary', 'alt', 'shift'],
        'key': 'arrow_right',
      },
      {
        'action': kShortcutActionScreenshot,
        'mods': ['primary', 'alt', 'shift'],
        'key': 'p',
      },
      {
        'action': kShortcutActionSwitchTabNext,
        'mods': ['primary', 'alt', 'shift'],
        'key': 'right_bracket',
      },
      {
        'action': kShortcutActionToggleRelativeMouseMode,
        'mods': ['primary', 'alt', 'shift'],
        'key': 'g',
      },
    ];

    final filtered = filterDefaultBindingsForPlatform(
      defaults,
      capabilities(
        includeFullscreenShortcut: false,
        includeScreenshotShortcut: false,
        includeTabShortcuts: false,
        includeToolbarShortcut: false,
        includeCloseTabShortcut: false,
        includeSwitchSidesShortcut: false,
        includeRecordingShortcut: false,
        includeResetCanvasShortcut: false,
        includePinToolbarShortcut: false,
        includeViewModeShortcut: false,
        includeInputSourceShortcut: false,
        includeVoiceCallShortcut: false,
      ),
    );

    expect(filtered.map((binding) => binding['action']), [
      kShortcutActionSendCtrlAltDel,
      kShortcutActionSwitchDisplayNext,
    ]);
  });

  Set<String> idSet(Iterable<KeyboardShortcutActionGroup> groups) =>
      {for (final e in allActionEntries(groups)) e.id};

  /// Convenience: extract the children of the named group as a flat list of
  /// human-readable tokens. Subgroups appear as `'group:<title>'` followed
  /// by their entries, so call sites can assert on full ordering (subgroups
  /// interleaved with direct items) in one expectation.
  List<String> childTokens(
      List<KeyboardShortcutActionGroup> groups, String titleKey) {
    final group = groups.firstWhere((g) => g.titleKey == titleKey);
    final out = <String>[];
    for (final child in group.children) {
      switch (child) {
        case KeyboardShortcutActionEntry():
          out.add(child.id);
        case KeyboardShortcutActionSubgroup():
          out.add('group:${child.titleKey}');
          for (final entry in child.entries) {
            out.add('  ${entry.id}');
          }
      }
    }
    return out;
  }

  test('filterKeyboardShortcutActionGroupsForPlatform strips desktop-only', () {
    final groups = filterKeyboardShortcutActionGroupsForPlatform(
      capabilities(
        includeFullscreenShortcut: false,
        includeScreenshotShortcut: false,
        includeTabShortcuts: false,
        includeToolbarShortcut: false,
        includeCloseTabShortcut: false,
        includeSwitchSidesShortcut: false,
        // Recording / Reset Canvas are intentionally still included here —
        // they have non-desktop platforms (mobile Android / mobile both).
        includeRecordingShortcut: true,
        includeResetCanvasShortcut: true,
        includePinToolbarShortcut: false,
        includeViewModeShortcut: false,
        includeInputSourceShortcut: false,
        includeVoiceCallShortcut: false,
      ),
    );
    final ids = idSet(groups);
    // Desktop-only actions are stripped.
    expect(ids, isNot(contains(kShortcutActionToggleFullscreen)));
    expect(ids, isNot(contains(kShortcutActionToggleRelativeMouseMode)));
    expect(ids, isNot(contains(kShortcutActionScreenshot)));
    expect(ids, isNot(contains(kShortcutActionToggleToolbar)));
    expect(ids, isNot(contains(kShortcutActionCloseTab)));
    expect(ids, isNot(contains(kShortcutActionSwitchSides)));
    expect(ids, isNot(contains(kShortcutActionPinToolbar)));
    expect(ids, isNot(contains(kShortcutActionViewModeOriginal)));
    expect(ids, isNot(contains(kShortcutActionViewModeAdaptive)));
    expect(ids, isNot(contains(kShortcutActionSwitchTabNext)));
    expect(ids, isNot(contains(kShortcutActionSwitchTabPrev)));
    // Cross-platform actions survive.
    expect(ids, contains(kShortcutActionSendCtrlAltDel));
    expect(ids, contains(kShortcutActionInsertLock));
    expect(ids, contains(kShortcutActionRestartRemote));
    expect(ids, contains(kShortcutActionSwitchDisplayNext));
    expect(ids, contains(kShortcutActionToggleRecording));
    expect(ids, contains(kShortcutActionResetCanvas));
    expect(ids, contains(kShortcutActionToggleMute));
  });

  test(
      'filterKeyboardShortcutActionGroupsForPlatform hides Toggle Recording on Web/iOS',
      () {
    final groups = filterKeyboardShortcutActionGroupsForPlatform(
      capabilities(includeRecordingShortcut: false),
    );
    final ids = idSet(groups);
    expect(ids, isNot(contains(kShortcutActionToggleRecording)));
    // Other Session Control entries unaffected.
    expect(ids, contains(kShortcutActionSendCtrlAltDel));
    expect(ids, contains(kShortcutActionInsertLock));
  });

  test(
      'filterKeyboardShortcutActionGroupsForPlatform keeps full set on desktop',
      () {
    final groups =
        filterKeyboardShortcutActionGroupsForPlatform(capabilities());
    expect(idSet(groups), equals(idSet(kKeyboardShortcutActionGroups)));
  });

  test('shortcut action groups follow toolbar menu order', () {
    final groups = kKeyboardShortcutActionGroups;

    // Top-level groups in toolbar order.
    expect(
      groups.map((g) => g.titleKey).toList(),
      ['Monitor', 'Control Actions', 'Display', 'Keyboard', 'Chat', 'Other'],
    );

    // Display: subgroups (View Mode → Image Quality → Codec → Virtual
    // display) first, then direct items (cursor toggles + display toggles),
    // then Privacy mode subgroup last — exactly matching `_DisplayMenu`.
    expect(childTokens(groups, 'Display'), [
      'group:View Mode',
      '  $kShortcutActionViewModeOriginal',
      '  $kShortcutActionViewModeAdaptive',
      '  $kShortcutActionViewModeCustom',
      'group:Image Quality',
      '  $kShortcutActionImageQualityBest',
      '  $kShortcutActionImageQualityBalanced',
      '  $kShortcutActionImageQualityLow',
      'group:Codec',
      '  $kShortcutActionCodecAuto',
      '  $kShortcutActionCodecVp8',
      '  $kShortcutActionCodecVp9',
      '  $kShortcutActionCodecAv1',
      '  $kShortcutActionCodecH264',
      '  $kShortcutActionCodecH265',
      'group:Virtual display',
      '  $kShortcutActionPlugOutAllVirtualDisplays',
      kShortcutActionToggleShowRemoteCursor,
      kShortcutActionToggleFollowRemoteCursor,
      kShortcutActionToggleFollowRemoteWindow,
      kShortcutActionToggleZoomCursor,
      kShortcutActionToggleQualityMonitor,
      kShortcutActionToggleMute,
      kShortcutActionToggleEnableFileCopyPaste,
      kShortcutActionToggleDisableClipboard,
      kShortcutActionToggleLockAfterSessionEnd,
      kShortcutActionToggleTrueColor,
      'group:Privacy mode',
      '  $kShortcutActionPrivacyMode1',
      '  $kShortcutActionPrivacyMode2',
    ]);

    // Privacy mode is the last child under Display (matching the toolbar's
    // submenu order — `_DisplayMenu` adds Privacy mode after the toggles).
    final displayChildren =
        groups.firstWhere((g) => g.titleKey == 'Display').children;
    expect(displayChildren.last, isA<KeyboardShortcutActionSubgroup>());
    expect(
      (displayChildren.last as KeyboardShortcutActionSubgroup).titleKey,
      'Privacy mode',
    );

    // Keyboard: Keyboard mode subgroup first, then direct items —
    // matching `_KeyboardMenu`.
    expect(childTokens(groups, 'Keyboard'), [
      'group:Keyboard mode',
      '  $kShortcutActionKeyboardModeLegacy',
      '  $kShortcutActionKeyboardModeMap',
      '  $kShortcutActionKeyboardModeTranslate',
      kShortcutActionToggleInputSource,
      kShortcutActionToggleViewOnly,
      kShortcutActionToggleShowMyCursor,
      kShortcutActionToggleSwapCtrlCmd,
      kShortcutActionToggleRelativeMouseMode,
      kShortcutActionToggleReverseMouseWheel,
      kShortcutActionToggleSwapLeftRightMouse,
    ]);
  });

  test('filterKeyboardShortcutActionGroupsForPlatform drops empty groups', () {
    // Sanity: KeyboardShortcutActionGroup ctor still accepts a single direct
    // entry as a child.
    final original = [
      KeyboardShortcutActionGroup('TestGroup', [
        KeyboardShortcutActionEntry(kShortcutActionCloseTab, 'Close Tab'),
      ]),
    ];
    expect(original.first.children, hasLength(1));

    // With every capability flag off, groups whose items are all behind
    // those flags get dropped. Display / Keyboard parent groups still carry
    // cross-platform direct items so they survive even when the gated
    // subgroups thin out.
    final groups = filterKeyboardShortcutActionGroupsForPlatform(
      capabilities(
        includeFullscreenShortcut: false,
        includeScreenshotShortcut: false,
        includeTabShortcuts: false,
        includeToolbarShortcut: false,
        includeCloseTabShortcut: false,
        includeSwitchSidesShortcut: false,
        includeRecordingShortcut: false,
        includeResetCanvasShortcut: false,
        includePinToolbarShortcut: false,
        includeViewModeShortcut: false,
        includeInputSourceShortcut: false,
        includeVoiceCallShortcut: false,
      ),
    );
    final titles = groups.map((g) => g.titleKey).toList();
    // "Other" has nothing but platform-gated entries → dropped entirely.
    expect(titles, isNot(contains('Other')));
    // Parent groups with cross-platform direct items survive.
    expect(titles, contains('Display'));
    expect(titles, contains('Keyboard'));
    // The "View Mode" subgroup under Display is gated by includeViewModeShortcut,
    // so it must be absent from Display's surviving children.
    final displayChildren =
        groups.firstWhere((g) => g.titleKey == 'Display').children;
    final subgroupTitles = displayChildren
        .whereType<KeyboardShortcutActionSubgroup>()
        .map((s) => s.titleKey)
        .toList();
    expect(subgroupTitles, isNot(contains('View Mode')));
    // No surviving group is empty either way.
    expect(groups.every((g) => g.children.isNotEmpty), isTrue);
    // No surviving subgroup is empty.
    for (final group in groups) {
      for (final child in group.children) {
        if (child is KeyboardShortcutActionSubgroup) {
          expect(child.entries, isNotEmpty,
              reason: 'subgroup "${child.titleKey}" should not be empty');
        }
      }
    }
  });

  test('logicalKeyName covers the supported-keys fixture', () {
    // The fixture is the cross-language source of truth for the full set of
    // shortcut-bindable key names. Rust has a mirror test against the same
    // file (`supported_keys_match_fixture` in src/keyboard/shortcuts.rs).
    // Drift on either side breaks one of the two tests.
    final fixturePath = 'test/fixtures/supported_shortcut_keys.json';
    final fixture =
        (jsonDecode(File(fixturePath).readAsStringSync()) as List<dynamic>)
            .cast<String>()
            .toSet();

    // Hand-rolled (LogicalKeyboardKey, name) round-trip table. Adding a key
    // requires updates in three places: the fixture, this table, and Rust's
    // matching table — that's the price of the parity guarantee.
    final mappings = <(LogicalKeyboardKey, String)>[
      for (var c = 0; c < 26; c++)
        (
          LogicalKeyboardKey(0x00000000061 + c),
          String.fromCharCode(0x61 + c),
        ),
      for (var d = 0; d < 10; d++)
        (LogicalKeyboardKey(0x00000000030 + d), 'digit$d'),
      (LogicalKeyboardKey.f1, 'f1'),
      (LogicalKeyboardKey.f2, 'f2'),
      (LogicalKeyboardKey.f3, 'f3'),
      (LogicalKeyboardKey.f4, 'f4'),
      (LogicalKeyboardKey.f5, 'f5'),
      (LogicalKeyboardKey.f6, 'f6'),
      (LogicalKeyboardKey.f7, 'f7'),
      (LogicalKeyboardKey.f8, 'f8'),
      (LogicalKeyboardKey.f9, 'f9'),
      (LogicalKeyboardKey.f10, 'f10'),
      (LogicalKeyboardKey.f11, 'f11'),
      (LogicalKeyboardKey.f12, 'f12'),
      (LogicalKeyboardKey.delete, 'delete'),
      (LogicalKeyboardKey.backspace, 'backspace'),
      (LogicalKeyboardKey.tab, 'tab'),
      (LogicalKeyboardKey.space, 'space'),
      (LogicalKeyboardKey.enter, 'enter'),
      (LogicalKeyboardKey.numpadEnter, 'enter'),
      (LogicalKeyboardKey.arrowLeft, 'arrow_left'),
      (LogicalKeyboardKey.arrowRight, 'arrow_right'),
      (LogicalKeyboardKey.arrowUp, 'arrow_up'),
      (LogicalKeyboardKey.arrowDown, 'arrow_down'),
      (LogicalKeyboardKey.home, 'home'),
      (LogicalKeyboardKey.end, 'end'),
      (LogicalKeyboardKey.pageUp, 'page_up'),
      (LogicalKeyboardKey.pageDown, 'page_down'),
      (LogicalKeyboardKey.insert, 'insert'),
    ];

    // Round-trip: every (key, name) pair must agree with logicalKeyName.
    for (final (key, name) in mappings) {
      expect(logicalKeyName(key), equals(name),
          reason: 'logicalKeyName($key) should be "$name"');
    }

    // The set of names produced by the table must equal the fixture.
    final namesFromTable = mappings.map((e) => e.$2).toSet();
    expect(namesFromTable, equals(fixture),
        reason: 'logicalKeyName vocabulary drifted from $fixturePath — update '
            'shortcut_utils.dart::logicalKeyName, the fixture, and Rust '
            'event_to_key_name together');

    // Modifier-only / unsupported keys must return null.
    expect(logicalKeyName(LogicalKeyboardKey.shift), isNull);
    expect(logicalKeyName(LogicalKeyboardKey.escape), isNull);
    expect(logicalKeyName(LogicalKeyboardKey.f13), isNull);
  });

  test('configurable shortcut list does not include known-removed action IDs',
      () {
    // These IDs were briefly defined without handlers (a "ghost action"
    // footgun). If you intend to re-add one of these as a real action,
    // wire up its handler and add a constant + group entry — do not just
    // resurrect the literal string below.
    //
    // Note: `toggle_privacy_mode` was once on this list but is now a real
    // implemented action (registered in shortcut_model.dart). The other
    // legacy IDs (toggle_audio, view_mode_shrink/stretch, view_mode_1_to_1)
    // were renamed: their replacements are kShortcutActionToggleMute and
    // kShortcutActionViewModeOriginal/Adaptive/Custom.
    const knownRemoved = [
      'toggle_audio',
      'view_mode_1_to_1',
      'view_mode_shrink',
      'view_mode_stretch',
    ];
    final actions = idSet(kKeyboardShortcutActionGroups);
    for (final id in knownRemoved) {
      expect(actions, isNot(contains(id)),
          reason:
              '"$id" was a known ghost action — wire a real handler before re-adding it');
    }
  });
}
