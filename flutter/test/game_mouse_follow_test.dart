import 'package:flutter_hbb/utils/game_mouse_follow.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('explicit choices survive reconnect and are isolated by peer', () {
    final saved = <String, bool>{};
    GameMouseFollow connect(String peer) => GameMouseFollow(
      readPreference: () => saved[peer] ?? false,
      writePreference: (v) => saved[peer] = v,
    );
    final lab = connect('lab');
    expect(lab.updateVisibility(false), isFalse);
    lab.enable(true, remember: true);
    lab.enable(false); // emergency release, local to this session
    expect(lab.updateVisibility(true), isFalse);
    lab.reset();
    expect(lab.updateVisibility(false), isTrue);
    expect(lab.pending, isTrue);
    expect(connect('other').updateVisibility(false), isFalse);
    final next = connect('lab');
    expect(next.updateVisibility(true), isTrue);
    expect(next.pending, isFalse);
    next.enable(false, remember: true);
    expect(connect('lab').updateVisibility(false), isFalse);
  });

  test('only a fresh hide captures the mouse after a manual release', () {
    final state = GameMouseFollow();
    expect(state.updateVisibility(false), isFalse);
    state.enable(true);
    expect(state.pending, isTrue);
    state.engaged();
    expect(state.updateVisibility(false), isFalse);
    expect(state.pending, isFalse);
    expect(state.updateVisibility(true), isTrue);
    expect(state.pending, isFalse);
    expect(state.updateVisibility(false), isTrue);
    expect(state.pending, isTrue);
    state.reset();
    expect(state.visible, isNull);
    expect(state.pending, isFalse);
    expect(state.updateVisibility(false), isFalse);
  });
  test('Alt temporarily releases a hidden cursor and respects a visible menu on key-up', () {
    final state = GameMouseFollow()..updateVisibility(false)..enable(true)..engaged();
    expect(state.updateAlt(true), isTrue);
    expect(state.wantsPointer, isTrue);
    expect(state.pending, isFalse);
    state.updateVisibility(true);
    state.updateAlt(false);
    expect(state.pending, isFalse);
    state.updateVisibility(false);
    state.engaged();
    state.updateAlt(true);
    state.updateAlt(false);
    expect(state.pending, isTrue);
    state.reset();
    expect(state.updateAlt(true), isFalse);
  });
}
