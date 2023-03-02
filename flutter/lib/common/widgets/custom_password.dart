// https://github.com/rodrigobastosv/fancy_password_field
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';
import 'package:password_strength/password_strength.dart';

abstract class ValidationRule {
  String get name;
  bool validate(String value);
}

class UppercaseValidationRule extends ValidationRule {
  @override
  String get name => translate('uppercase');
  @override
  bool validate(String value) {
    return value.contains(RegExp(r'[A-Z]'));
  }
}

class LowercaseValidationRule extends ValidationRule {
  @override
  String get name => translate('lowercase');

  @override
  bool validate(String value) {
    return value.contains(RegExp(r'[a-z]'));
  }
}

class DigitValidationRule extends ValidationRule {
  @override
  String get name => translate('digit');

  @override
  bool validate(String value) {
    return value.contains(RegExp(r'[0-9]'));
  }
}

class SpecialCharacterValidationRule extends ValidationRule {
  @override
  String get name => translate('special character');

  @override
  bool validate(String value) {
    return value.contains(RegExp(r'[!@#$%^&*(),.?":{}|<>]'));
  }
}

class MinCharactersValidationRule extends ValidationRule {
  final int _numberOfCharacters;
  MinCharactersValidationRule(this._numberOfCharacters);

  @override
  String get name => translate('length>=$_numberOfCharacters');

  @override
  bool validate(String value) {
    return value.length >= _numberOfCharacters;
  }
}

class PasswordStrengthIndicator extends StatelessWidget {
  final RxString password;
  final double weakMedium = 0.33;
  final double mediumStrong = 0.67;
  const PasswordStrengthIndicator({Key? key, required this.password})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      var strength = estimatePasswordStrength(password.value);
      return Row(
        children: [
          Expanded(
              child: _indicator(
                  password.isEmpty ? Colors.grey : _getColor(strength))),
          Expanded(
              child: _indicator(password.isEmpty || strength < weakMedium
                  ? Colors.grey
                  : _getColor(strength))),
          Expanded(
              child: _indicator(password.isEmpty || strength < mediumStrong
                  ? Colors.grey
                  : _getColor(strength))),
          Text(password.isEmpty ? '' : translate(_getLabel(strength)))
              .marginOnly(left: password.isEmpty ? 0 : 8),
        ],
      );
    });
  }

  Widget _indicator(Color color) {
    return Container(
      height: 8,
      color: color,
    );
  }

  String _getLabel(double strength) {
    if (strength < weakMedium) {
      return 'Weak';
    } else if (strength < mediumStrong) {
      return 'Medium';
    } else {
      return 'Strong';
    }
  }

  Color _getColor(double strength) {
    if (strength < weakMedium) {
      return Colors.yellow;
    } else if (strength < mediumStrong) {
      return Colors.blue;
    } else {
      return Colors.green;
    }
  }
}
