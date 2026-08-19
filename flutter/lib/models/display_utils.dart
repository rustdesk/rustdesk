import '../consts.dart';

/// Compute the next individual display index in a round-robin cycle.
///
/// The "All displays" pseudo-monitor (`kAllDisplayValue`) is skipped: cycling
/// from it wraps to the first individual display. Callers must ensure
/// `total > 1`; with fewer displays there is nothing to cycle between.
int nextDisplayIndex(int current, int total) {
  return current == kAllDisplayValue ? 0 : (current + 1) % total;
}
