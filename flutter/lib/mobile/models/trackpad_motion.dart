import 'dart:math';

const kOptionTrackpadAcceleration = 'trackpad-acceleration';
const kOptionTrackpadGlide = 'trackpad-glide';

double trackpadMotionStrength(String value) =>
    (int.tryParse(value) ?? 50).clamp(0, 100) / 100;

double trackpadFilteredSpeed(
    double previousSpeed, double distance, Duration elapsed) {
  if (elapsed.isNegative || elapsed > const Duration(milliseconds: 80)) return 0;
  if (elapsed == Duration.zero) return previousSpeed;

  final speed = min(
      distance * Duration.microsecondsPerSecond / elapsed.inMicroseconds,
      1600.0);
  // Time-based smoothing keeps the response similar across touch sample rates.
  final responseTime = speed > previousSpeed ? 16000 : 24000;
  final weight = 1 - exp(-elapsed.inMicroseconds / responseTime);
  return previousSpeed + (speed - previousSpeed) * weight;
}

double trackpadMotionGain(double speed, double strength) {
  final fraction = ((speed - 180) / 900).clamp(0.0, 1.0);
  final smoothGain = fraction * fraction * (3 - 2 * fraction);
  return 1 + smoothGain * 2.3 * strength;
}

Duration trackpadGlideDuration(double strength) =>
    Duration(milliseconds: (1000 * strength).round());

double trackpadGlideDistance(
    double speed, Duration sinceLastMove, Duration duration) {
  if (sinceLastMove.isNegative ||
      sinceLastMove > const Duration(milliseconds: 55)) {
    return 0;
  }
  final glideSpeed = (speed - 650).clamp(0.0, 1500.0);
  return glideSpeed * (duration.inMilliseconds / 2500);
}

double trackpadGlideProgress(Duration elapsed, Duration duration) {
  final remaining =
      1 - (elapsed.inMicroseconds / duration.inMicroseconds).clamp(0.0, 1.0);
  return 1 - remaining * remaining;
}
