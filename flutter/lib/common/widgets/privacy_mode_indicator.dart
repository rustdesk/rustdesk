import 'package:flutter/material.dart';
import 'package:get/get.dart';

class PrivacyModeIndicator extends StatelessWidget {
  const PrivacyModeIndicator({
    Key? key,
    required this.state,
    required this.color,
    required this.semanticsLabel,
  }) : super(key: key);

  final RxString state;
  final Color color;
  final String semanticsLabel;

  @override
  Widget build(BuildContext context) => Obx(
        () => state.isEmpty
            ? const SizedBox.shrink()
            : IgnorePointer(
                child: Semantics(
                  image: true,
                  label: semanticsLabel,
                  child: Container(
                    width: 32,
                    height: 32,
                    decoration: BoxDecoration(
                      color: color.withOpacity(0.92),
                      shape: BoxShape.circle,
                    ),
                    child: const Icon(
                      Icons.privacy_tip,
                      color: Colors.white,
                      size: 20,
                    ),
                  ),
                ),
              ),
      );
}
