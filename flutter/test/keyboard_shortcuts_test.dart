import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_actions.dart';
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_constants.dart';
import 'package:flutter_hbb/common/widgets/keyboard_shortcuts/shortcut_utils.dart';
import 'package:flutter_hbb/models/shortcut_model.dart';

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

  test('shortcutBindingMapsFrom ignores malformed binding fields', () {
    final valid = {
      'action': kShortcutActionScreenshot,
      'mods': ['primary', 'ctrl', 'alt', 'shift'],
      'key': 'p',
    };
    final malformed = [
      for (final field in ['action', 'mods', 'key'])
        {...valid}..remove(field),
      for (final fields in [
        {'action': 1},
        {'action': null},
        {'key': 1},
        {'key': null},
        {'mods': null},
        {'mods': 'primary'},
        {'mods': ['primary', 1]},
        {'mods': ['primary', null]},
        {'mods': ['primary', 'unknown']},
      ])
        {...valid, ...fields},
    ];

    for (final binding in malformed) {
      expect(shortcutBindingMapsFrom([binding, valid]), [valid],
          reason: 'malformed binding should be ignored: $binding');
    }
  });

  test('shortcutBindingMapsFrom preserves unbound and unknown bindings', () {
    expect(
      shortcutBindingMapsFrom([
        {'action': kShortcutActionScreenshot, 'mods': [], 'key': ''},
        {'action': 'future_action', 'mods': ['ctrl'], 'key': 'future_key'},
      ]),
      [
        {'action': kShortcutActionScreenshot, 'mods': [], 'key': ''},
        {'action': 'future_action', 'mods': ['ctrl'], 'key': 'future_key'},
      ],
    );
  });

  test('platform filtering ignores malformed binding fields', () {
    final filtered = filterDefaultBindingsForPlatform([
      {'action': 1, 'mods': ['primary'], 'key': 'p'},
      {'action': kShortcutActionScreenshot, 'mods': ['primary'], 'key': 1},
      {'action': kShortcutActionToggleMute, 'mods': ['alt'], 'key': 's'},
      {
        'action': kShortcutActionToggleFullscreen,
        'mods': ['primary'],
        'key': 'enter',
      },
    ], capabilities(includeFullscreenShortcut: false));

    expect(filtered, [
      {'action': kShortcutActionToggleMute, 'mods': ['alt'], 'key': 's'},
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

  test('physicalKeyName covers the supported-keys fixture', () {
    // `shortcut_key_usb_hid.json` pins every key name to the USB HID usage
    // it is recorded and matched from. Rust has a mirror test against the
    // same file (`usb_hid_keys_match_fixture` in src/keyboard/shortcuts.rs),
    // and the set of names must equal `supported_shortcut_keys.json`.
    final supported = (jsonDecode(
                File('test/fixtures/supported_shortcut_keys.json')
                    .readAsStringSync()) as List<dynamic>)
        .cast<String>()
        .toSet();
    final pairs = (jsonDecode(File('test/fixtures/shortcut_key_usb_hid.json')
            .readAsStringSync()) as List<dynamic>)
        .cast<List<dynamic>>();

    for (final pair in pairs) {
      final name = pair[0] as String;
      final usage = pair[1] as int;
      expect(physicalKeyName(PhysicalKeyboardKey(0x00070000 | usage)),
          equals(name),
          reason: 'USB HID 0x${usage.toRadixString(16)} should be "$name"');
    }
    expect(pairs.map((p) => p[0] as String).toSet(), equals(supported));

    // Modifier-only / unsupported keys must return null.
    expect(physicalKeyName(PhysicalKeyboardKey.shiftLeft), isNull);
    expect(physicalKeyName(PhysicalKeyboardKey.escape), isNull);
    expect(physicalKeyName(PhysicalKeyboardKey.f13), isNull);
    expect(physicalKeyName(PhysicalKeyboardKey.numpad1), isNull);
  });

  test('enable seeds defaults only for a config without bindings', () {
    expect(shouldSeedDefaultShortcutBindings(<String, dynamic>{}), isTrue);
    expect(
        shouldSeedDefaultShortcutBindings(
            <String, dynamic>{'enabled': false, 'pass_through': true}),
        isTrue);
    // The user cleared every binding: re-enabling must not bring defaults back.
    expect(
        shouldSeedDefaultShortcutBindings(
            <String, dynamic>{'enabled': false, 'bindings': <dynamic>[]}),
        isFalse);
  });

  test('ShortcutConfig.parse reads the flags and bindings once', () {
    final config = ShortcutConfig.parse(jsonEncode({
      'enabled': true,
      'pass_through': false,
      'bindings': [
        {'action': 'screenshot', 'mods': ['primary'], 'key': 'p'},
        'not a binding',
      ],
    }));
    expect(config.enabled, isTrue);
    expect(config.passThrough, isFalse);
    expect(config.bindings, [
      {'action': 'screenshot', 'mods': ['primary'], 'key': 'p'},
    ]);

    for (final raw in ['', 'not json', '[]', '{"bindings": "x"}']) {
      final broken = ShortcutConfig.parse(raw);
      expect(broken.enabled, isFalse, reason: raw);
      expect(broken.passThrough, isFalse, reason: raw);
      expect(broken.bindings, isEmpty, reason: raw);
    }
  });

  test('non-US layouts record and match the physical key', () {
    // AZERTY: the key labelled "A" sits where US QWERTY has Q. The native
    // matcher only sees the physical position (USB HID usage), so the
    // binding must name that position too.
    const azertyA = KeyDownEvent(
      physicalKey: PhysicalKeyboardKey.keyQ,
      logicalKey: LogicalKeyboardKey.keyA,
      character: 'a',
      timeStamp: Duration.zero,
    );
    expect(shortcutKeyNameForEvent(azertyA), 'q');

    // QWERTZ: the key labelled "Z" sits where US QWERTY has Y.
    const qwertzZ = KeyDownEvent(
      physicalKey: PhysicalKeyboardKey.keyY,
      logicalKey: LogicalKeyboardKey.keyZ,
      character: 'z',
      timeStamp: Duration.zero,
    );
    expect(shortcutKeyNameForEvent(qwertzZ), 'y');
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
    //
    // `toggle_input_source` was removed on purpose: it switches the key
    // capture backend between key down and key up, so the matcher that
    // consumed the press never sees the repeats and the release.
    const knownRemoved = [
      'toggle_audio',
      'toggle_input_source',
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
