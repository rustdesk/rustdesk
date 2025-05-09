import 'dart:async';
import 'dart:convert';

import 'package:bot_toast/bot_toast.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/setting_widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/peer_tab_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import 'address_book.dart';

void clientClose(SessionID sessionId, OverlayDialogManager dialogManager) {
  msgBox(sessionId, 'info', 'Close', 'Are you sure to close the connection?',
      '', dialogManager);
}

abstract class ValidationRule {
  String get name;
  bool validate(String value);
}

class LengthRangeValidationRule extends ValidationRule {
  final int _min;
  final int _max;

  LengthRangeValidationRule(this._min, this._max);

  @override
  String get name => translate('length %min% to %max%')
      .replaceAll('%min%', _min.toString())
      .replaceAll('%max%', _max.toString());

  @override
  bool validate(String value) {
    return value.length >= _min && value.length <= _max;
  }
}

class RegexValidationRule extends ValidationRule {
  final String _name;
  final RegExp _regex;

  RegexValidationRule(this._name, this._regex);

  @override
  String get name => translate(_name);

  @override
  bool validate(String value) {
    return value.isNotEmpty ? value.contains(_regex) : false;
  }
}

void changeIdDialog() {
  var newId = "";
  var msg = "";
  var isInProgress = false;
  TextEditingController controller = TextEditingController();
  final RxString rxId = controller.text.trim().obs;

  final rules = [
    RegexValidationRule('starts with a letter', RegExp(r'^[a-zA-Z]')),
    LengthRangeValidationRule(6, 16),
    RegexValidationRule('allowed characters', RegExp(r'^[\w-]*$'))
  ];

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      debugPrint("onSubmit");
      newId = controller.text.trim();

      final Iterable violations = rules.where((r) => !r.validate(newId));
      if (violations.isNotEmpty) {
        setState(() {
          msg = (isDesktop || isWebDesktop)
              ? '${translate('Prompt')}:  ${violations.map((r) => r.name).join(', ')}'
              : violations.map((r) => r.name).join(', ');
        });
        return;
      }

      setState(() {
        msg = "";
        isInProgress = true;
        bind.mainChangeId(newId: newId);
      });

      var status = await bind.mainGetAsyncStatus();
      while (status == " ") {
        await Future.delayed(const Duration(milliseconds: 100));
        status = await bind.mainGetAsyncStatus();
      }
      if (status.isEmpty) {
        // ok
        close();
        return;
      }
      setState(() {
        isInProgress = false;
        msg = (isDesktop || isWebDesktop)
            ? '${translate('Prompt')}: ${translate(status)}'
            : translate(status);
      });
    }

    return CustomAlertDialog(
      title: Text(translate("Change ID")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("id_change_tip")),
          const SizedBox(
            height: 12.0,
          ),
          TextField(
            decoration: InputDecoration(
                labelText: translate('Your new ID'),
                errorText: msg.isEmpty ? null : translate(msg),
                suffixText: '${rxId.value.length}/16',
                suffixStyle: const TextStyle(fontSize: 12, color: Colors.grey)),
            inputFormatters: [
              LengthLimitingTextInputFormatter(16),
              // FilteringTextInputFormatter(RegExp(r"[a-zA-z][a-zA-z0-9\_]*"), allow: true)
            ],
            controller: controller,
            autofocus: true,
            onChanged: (value) {
              setState(() {
                rxId.value = value.trim();
                msg = '';
              });
            },
          ).workaroundFreezeLinuxMint(),
          const SizedBox(
            height: 8.0,
          ),
          (isDesktop || isWebDesktop)
              ? Obx(() => Wrap(
                    runSpacing: 8,
                    spacing: 4,
                    children: rules.map((e) {
                      var checked = e.validate(rxId.value);
                      return Chip(
                          label: Text(
                            e.name,
                            style: TextStyle(
                                color: checked
                                    ? const Color(0xFF0A9471)
                                    : Color.fromARGB(255, 198, 86, 157)),
                          ),
                          backgroundColor: checked
                              ? const Color(0xFFD0F7ED)
                              : Color.fromARGB(255, 247, 205, 232));
                    }).toList(),
                  )).marginOnly(bottom: 8)
              : SizedBox.shrink(),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void changeWhiteList({Function()? callback}) async {
  final curWhiteList = await bind.mainGetOption(key: kOptionWhitelist);
  var newWhiteListField = curWhiteList == defaultOptionWhitelist
      ? ''
      : curWhiteList.split(',').join('\n');
  var controller = TextEditingController(text: newWhiteListField);
  var msg = "";
  var isInProgress = false;
  final isOptFixed = isOptionFixed(kOptionWhitelist);
  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("IP Whitelisting")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("whitelist_sep")),
          const SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        decoration: InputDecoration(
                          errorText: msg.isEmpty ? null : translate(msg),
                        ),
                        controller: controller,
                        enabled: !isOptFixed,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
          const SizedBox(
            height: 4.0,
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        if (!isOptFixed)
          dialogButton("Clear", onPressed: () async {
            await bind.mainSetOption(
                key: kOptionWhitelist, value: defaultOptionWhitelist);
            callback?.call();
            close();
          }, isOutline: true),
        if (!isOptFixed)
          dialogButton(
            "OK",
            onPressed: () async {
              setState(() {
                msg = "";
                isInProgress = true;
              });
              newWhiteListField = controller.text.trim();
              var newWhiteList = "";
              if (newWhiteListField.isEmpty) {
                // pass
              } else {
                final ips =
                    newWhiteListField.trim().split(RegExp(r"[\s,;\n]+"));
                // test ip
                final ipMatch = RegExp(
                    r"^(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)(\/([1-9]|[1-2][0-9]|3[0-2])){0,1}$");
                final ipv6Match = RegExp(
                    r"^(((?:[0-9A-Fa-f]{1,4}))*((?::[0-9A-Fa-f]{1,4}))*::((?:[0-9A-Fa-f]{1,4}))*((?::[0-9A-Fa-f]{1,4}))*|((?:[0-9A-Fa-f]{1,4}))((?::[0-9A-Fa-f]{1,4})){7})(\/([1-9]|[1-9][0-9]|1[0-1][0-9]|12[0-8])){0,1}$");
                for (final ip in ips) {
                  if (!ipMatch.hasMatch(ip) && !ipv6Match.hasMatch(ip)) {
                    msg = "${translate("Invalid IP")} $ip";
                    setState(() {
                      isInProgress = false;
                    });
                    return;
                  }
                }
                newWhiteList = ips.join(',');
              }
              if (newWhiteList.trim().isEmpty) {
                newWhiteList = defaultOptionWhitelist;
              }
              await bind.mainSetOption(
                  key: kOptionWhitelist, value: newWhiteList);
              callback?.call();
              close();
            },
          ),
      ],
      onCancel: close,
    );
  });
}

Future<String> changeDirectAccessPort(
    String currentIP, String currentPort) async {
  final controller = TextEditingController(text: currentPort);
  await gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Change Local Port")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 8.0),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        keyboardType: TextInputType.number,
                        decoration: InputDecoration(
                            hintText: '21118',
                            isCollapsed: true,
                            prefix: Text('$currentIP : '),
                            suffix: IconButton(
                                padding: EdgeInsets.zero,
                                icon: const Icon(Icons.clear, size: 16),
                                onPressed: () => controller.clear())),
                        inputFormatters: [
                          FilteringTextInputFormatter.allow(RegExp(
                              r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                        ],
                        controller: controller,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () async {
          await bind.mainSetOption(
              key: kOptionDirectAccessPort, value: controller.text);
          close();
        }),
      ],
      onCancel: close,
    );
  });
  return controller.text;
}

Future<String> changeAutoDisconnectTimeout(String old) async {
  final controller = TextEditingController(text: old);
  await gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Timeout in minutes")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 8.0),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        keyboardType: TextInputType.number,
                        decoration: InputDecoration(
                            hintText: '10',
                            isCollapsed: true,
                            suffix: IconButton(
                                padding: EdgeInsets.zero,
                                icon: const Icon(Icons.clear, size: 16),
                                onPressed: () => controller.clear())),
                        inputFormatters: [
                          FilteringTextInputFormatter.allow(RegExp(
                              r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                        ],
                        controller: controller,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () async {
          await bind.mainSetOption(
              key: kOptionAutoDisconnectTimeout, value: controller.text);
          close();
        }),
      ],
      onCancel: close,
    );
  });
  return controller.text;
}

class DialogTextField extends StatelessWidget {
  final String title;
  final String? hintText;
  final bool obscureText;
  final String? errorText;
  final String? helperText;
  final Widget? prefixIcon;
  final Widget? suffixIcon;
  final TextEditingController controller;
  final FocusNode? focusNode;
  final TextInputType? keyboardType;
  final List<TextInputFormatter>? inputFormatters;
  final int? maxLength;

  static const kUsernameTitle = 'Username';
  static const kUsernameIcon = Icon(Icons.account_circle_outlined);
  static const kPasswordTitle = 'Password';
  static const kPasswordIcon = Icon(Icons.lock_outline);

  DialogTextField(
      {Key? key,
      this.focusNode,
      this.obscureText = false,
      this.errorText,
      this.helperText,
      this.prefixIcon,
      this.suffixIcon,
      this.hintText,
      this.keyboardType,
      this.inputFormatters,
      this.maxLength,
      required this.title,
      required this.controller})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: Column(
            children: [
              TextField(
                decoration: InputDecoration(
                  labelText: title,
                  hintText: hintText,
                  prefixIcon: prefixIcon,
                  suffixIcon: suffixIcon,
                  helperText: helperText,
                  helperMaxLines: 8,
                ),
                controller: controller,
                focusNode: focusNode,
                autofocus: true,
                obscureText: obscureText,
                keyboardType: keyboardType,
                inputFormatters: inputFormatters,
                maxLength: maxLength,
              ),
              if (errorText != null)
                Align(
                  alignment: Alignment.centerLeft,
                  child: SelectableText(
                    errorText!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                      fontSize: 12,
                    ),
                    textAlign: TextAlign.left,
                  ).paddingOnly(top: 8, left: 12),
                ),
            ],
          ).workaroundFreezeLinuxMint(),
        ),
      ],
    ).paddingSymmetric(vertical: 4.0);
  }
}

abstract class ValidationField extends StatelessWidget {
  ValidationField({Key? key}) : super(key: key);

  String? validate();
  bool get isReady;
}

class Dialog2FaField extends ValidationField {
  Dialog2FaField({
    Key? key,
    required this.controller,
    this.autoFocus = true,
    this.reRequestFocus = false,
    this.title,
    this.hintText,
    this.errorText,
    this.readyCallback,
    this.onChanged,
  }) : super(key: key);

  final TextEditingController controller;
  final bool autoFocus;
  final bool reRequestFocus;
  final String? title;
  final String? hintText;
  final String? errorText;
  final VoidCallback? readyCallback;
  final VoidCallback? onChanged;
  final errMsg = translate('2FA code must be 6 digits.');

  @override
  Widget build(BuildContext context) {
    return DialogVerificationCodeField(
      title: title ?? translate('2FA code'),
      controller: controller,
      errorText: errorText,
      autoFocus: autoFocus,
      reRequestFocus: reRequestFocus,
      hintText: hintText,
      readyCallback: readyCallback,
      onChanged: _onChanged,
      keyboardType: TextInputType.number,
      inputFormatters: [
        FilteringTextInputFormatter.allow(RegExp(r'[0-9]')),
      ],
    );
  }

  String get text => controller.text;
  bool get isAllDigits => text.codeUnits.every((e) => e >= 48 && e <= 57);

  @override
  bool get isReady => text.length == 6 && isAllDigits;

  @override
  String? validate() => isReady ? null : errMsg;

  _onChanged(StateSetter setState, SimpleWrapper<String?> errText) {
    onChanged?.call();

    if (text.length > 6) {
      setState(() => errText.value = errMsg);
      return;
    }

    if (!isAllDigits) {
      setState(() => errText.value = errMsg);
      return;
    }

    if (isReady) {
      readyCallback?.call();
      return;
    }

    if (errText.value != null) {
      setState(() => errText.value = null);
    }
  }
}

class DialogEmailCodeField extends ValidationField {
  DialogEmailCodeField({
    Key? key,
    required this.controller,
    this.autoFocus = true,
    this.reRequestFocus = false,
    this.hintText,
    this.errorText,
    this.readyCallback,
    this.onChanged,
  }) : super(key: key);

  final TextEditingController controller;
  final bool autoFocus;
  final bool reRequestFocus;
  final String? hintText;
  final String? errorText;
  final VoidCallback? readyCallback;
  final VoidCallback? onChanged;
  final errMsg = translate('Email verification code must be 6 characters.');

  @override
  Widget build(BuildContext context) {
    return DialogVerificationCodeField(
      title: translate('Verification code'),
      controller: controller,
      errorText: errorText,
      autoFocus: autoFocus,
      reRequestFocus: reRequestFocus,
      hintText: hintText,
      readyCallback: readyCallback,
      helperText: translate('verification_tip'),
      onChanged: _onChanged,
      keyboardType: TextInputType.visiblePassword,
    );
  }

  String get text => controller.text;

  @override
  bool get isReady => text.length == 6;

  @override
  String? validate() => isReady ? null : errMsg;

  _onChanged(StateSetter setState, SimpleWrapper<String?> errText) {
    onChanged?.call();

    if (text.length > 6) {
      setState(() => errText.value = errMsg);
      return;
    }

    if (isReady) {
      readyCallback?.call();
      return;
    }

    if (errText.value != null) {
      setState(() => errText.value = null);
    }
  }
}

class DialogVerificationCodeField extends StatefulWidget {
  DialogVerificationCodeField({
    Key? key,
    required this.controller,
    required this.title,
    this.autoFocus = true,
    this.reRequestFocus = false,
    this.helperText,
    this.hintText,
    this.errorText,
    this.textLength,
    this.readyCallback,
    this.onChanged,
    this.keyboardType,
    this.inputFormatters,
  }) : super(key: key);

  final TextEditingController controller;
  final bool autoFocus;
  final bool reRequestFocus;
  final String title;
  final String? helperText;
  final String? hintText;
  final String? errorText;
  final int? textLength;
  final VoidCallback? readyCallback;
  final Function(StateSetter setState, SimpleWrapper<String?> errText)?
      onChanged;
  final TextInputType? keyboardType;
  final List<TextInputFormatter>? inputFormatters;

  @override
  State<DialogVerificationCodeField> createState() =>
      _DialogVerificationCodeField();
}

class _DialogVerificationCodeField extends State<DialogVerificationCodeField> {
  final _focusNode = FocusNode();
  Timer? _timer;
  Timer? _timerReRequestFocus;
  SimpleWrapper<String?> errorText = SimpleWrapper(null);
  String _preText = '';

  @override
  void initState() {
    super.initState();
    if (widget.autoFocus) {
      _timer =
          Timer(Duration(milliseconds: 50), () => _focusNode.requestFocus());

      if (widget.onChanged != null) {
        widget.controller.addListener(() {
          final text = widget.controller.text.trim();
          if (text == _preText) return;
          widget.onChanged!(setState, errorText);
          _preText = text;
        });
      }
    }

    // software secure keyboard will take the focus since flutter 3.13
    // request focus again when android account password obtain focus
    if (isAndroid && widget.reRequestFocus) {
      _focusNode.addListener(() {
        if (_focusNode.hasFocus) {
          _timerReRequestFocus?.cancel();
          _timerReRequestFocus = Timer(
              Duration(milliseconds: 100), () => _focusNode.requestFocus());
        }
      });
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    _timerReRequestFocus?.cancel();
    _focusNode.unfocus();
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return DialogTextField(
      title: widget.title,
      controller: widget.controller,
      errorText: widget.errorText ?? errorText.value,
      focusNode: _focusNode,
      helperText: widget.helperText,
      keyboardType: widget.keyboardType,
      inputFormatters: widget.inputFormatters,
    );
  }
}

class PasswordWidget extends StatefulWidget {
  PasswordWidget({
    Key? key,
    required this.controller,
    this.autoFocus = true,
    this.reRequestFocus = false,
    this.hintText,
    this.errorText,
    this.title,
    this.maxLength,
  }) : super(key: key);

  final TextEditingController controller;
  final bool autoFocus;
  final bool reRequestFocus;
  final String? hintText;
  final String? errorText;
  final String? title;
  final int? maxLength;

  @override
  State<PasswordWidget> createState() => _PasswordWidgetState();
}

class _PasswordWidgetState extends State<PasswordWidget> {
  bool _passwordVisible = false;
  final _focusNode = FocusNode();
  Timer? _timer;
  Timer? _timerReRequestFocus;

  @override
  void initState() {
    super.initState();
    if (widget.autoFocus) {
      _timer =
          Timer(Duration(milliseconds: 50), () => _focusNode.requestFocus());
    }
    // software secure keyboard will take the focus since flutter 3.13
    // request focus again when android account password obtain focus
    if (isAndroid && widget.reRequestFocus) {
      _focusNode.addListener(() {
        if (_focusNode.hasFocus) {
          _timerReRequestFocus?.cancel();
          _timerReRequestFocus = Timer(
              Duration(milliseconds: 100), () => _focusNode.requestFocus());
        }
      });
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    _timerReRequestFocus?.cancel();
    _focusNode.unfocus();
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return DialogTextField(
      title: translate(widget.title ?? DialogTextField.kPasswordTitle),
      hintText: translate(widget.hintText ?? 'Enter your password'),
      controller: widget.controller,
      prefixIcon: DialogTextField.kPasswordIcon,
      suffixIcon: IconButton(
        icon: Icon(
            // Based on passwordVisible state choose the icon
            _passwordVisible ? Icons.visibility : Icons.visibility_off,
            color: MyTheme.lightTheme.primaryColor),
        onPressed: () {
          // Update the state i.e. toggle the state of passwordVisible variable
          setState(() {
            _passwordVisible = !_passwordVisible;
          });
        },
      ),
      obscureText: !_passwordVisible,
      errorText: widget.errorText,
      focusNode: _focusNode,
      maxLength: widget.maxLength,
    );
  }
}

void wrongPasswordDialog(SessionID sessionId,
    OverlayDialogManager dialogManager, type, title, text) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      enterPasswordDialog(sessionId, dialogManager);
    }

    return CustomAlertDialog(
        title: null,
        content: msgboxContent(type, title, text),
        onSubmit: submit,
        onCancel: cancel,
        actions: [
          dialogButton(
            'Cancel',
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            'Retry',
            onPressed: submit,
          ),
        ]);
  });
}

void enterPasswordDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  await _connectDialog(
    sessionId,
    dialogManager,
    passwordController: TextEditingController(),
  );
}

void enterUserLoginDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  await _connectDialog(
    sessionId,
    dialogManager,
    osUsernameController: TextEditingController(),
    osPasswordController: TextEditingController(),
  );
}

void enterUserLoginAndPasswordDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  await _connectDialog(
    sessionId,
    dialogManager,
    osUsernameController: TextEditingController(),
    osPasswordController: TextEditingController(),
    passwordController: TextEditingController(),
  );
}

_connectDialog(
  SessionID sessionId,
  OverlayDialogManager dialogManager, {
  TextEditingController? osUsernameController,
  TextEditingController? osPasswordController,
  TextEditingController? passwordController,
}) async {
  var rememberPassword = false;
  if (passwordController != null) {
    rememberPassword =
        await bind.sessionGetRemember(sessionId: sessionId) ?? false;
  }
  var rememberAccount = false;
  if (osUsernameController != null) {
    rememberAccount =
        await bind.sessionGetRemember(sessionId: sessionId) ?? false;
  }
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      final osUsername = osUsernameController?.text.trim() ?? '';
      final osPassword = osPasswordController?.text.trim() ?? '';
      final password = passwordController?.text.trim() ?? '';
      if (passwordController != null && password.isEmpty) return;
      if (rememberAccount) {
        bind.sessionPeerOption(
            sessionId: sessionId, name: 'os-username', value: osUsername);
        bind.sessionPeerOption(
            sessionId: sessionId, name: 'os-password', value: osPassword);
      }
      gFFI.login(
        osUsername,
        osPassword,
        sessionId,
        password,
        rememberPassword,
      );
      close();
      dialogManager.showLoading(translate('Logging in...'),
          onCancel: closeConnection);
    }

    descWidget(String text) {
      return Column(
        children: [
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              text,
              maxLines: 3,
              softWrap: true,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 16),
            ),
          ),
          Container(
            height: 8,
          ),
        ],
      );
    }

    rememberWidget(
      String desc,
      bool remember,
      ValueChanged<bool?>? onChanged,
    ) {
      return CheckboxListTile(
        contentPadding: const EdgeInsets.all(0),
        dense: true,
        controlAffinity: ListTileControlAffinity.leading,
        title: Text(desc),
        value: remember,
        onChanged: onChanged,
      );
    }

    osAccountWidget() {
      if (osUsernameController == null || osPasswordController == null) {
        return Offstage();
      }
      return Column(
        children: [
          descWidget(translate('login_linux_tip')),
          DialogTextField(
            title: translate(DialogTextField.kUsernameTitle),
            controller: osUsernameController,
            prefixIcon: DialogTextField.kUsernameIcon,
            errorText: null,
          ),
          PasswordWidget(
            controller: osPasswordController,
            autoFocus: false,
          ),
          rememberWidget(
            translate('remember_account_tip'),
            rememberAccount,
            (v) {
              if (v != null) {
                setState(() => rememberAccount = v);
              }
            },
          ),
        ],
      );
    }

    passwdWidget() {
      if (passwordController == null) {
        return Offstage();
      }
      return Column(
        children: [
          descWidget(translate('verify_rustdesk_password_tip')),
          PasswordWidget(
            controller: passwordController,
            autoFocus: osUsernameController == null,
          ),
          rememberWidget(
            translate('Remember password'),
            rememberPassword,
            (v) {
              if (v != null) {
                setState(() => rememberPassword = v);
              }
            },
          ),
        ],
      );
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.password_rounded, color: MyTheme.accent),
          Text(translate('Password Required')).paddingOnly(left: 10),
        ],
      ),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        osAccountWidget(),
        osUsernameController == null || passwordController == null
            ? Offstage()
            : Container(height: 12),
        passwdWidget(),
      ]),
      actions: [
        dialogButton(
          'Cancel',
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        dialogButton(
          'OK',
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}

void showWaitUacDialog(
    SessionID sessionId, OverlayDialogManager dialogManager, String type) {
  dialogManager.dismissAll();
  dialogManager.show(
      tag: '$sessionId-wait-uac',
      (setState, close, context) => CustomAlertDialog(
            title: null,
            content: msgboxContent(type, 'Wait', 'wait_accept_uac_tip'),
            actions: [
              dialogButton(
                'OK',
                icon: Icon(Icons.done_rounded),
                onPressed: close,
              ),
            ],
          ));
}

// Another username && password dialog?
void showRequestElevationDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) {
  RxString groupValue = ''.obs;
  RxString errUser = ''.obs;
  RxString errPwd = ''.obs;
  TextEditingController userController = TextEditingController();
  TextEditingController pwdController = TextEditingController();

  void onRadioChanged(String? value) {
    if (value != null) {
      groupValue.value = value;
    }
  }

  // TODO get from theme
  final double fontSizeNote = 13.00;

  Widget OptionRequestPermissions = Obx(
    () => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Radio(
          visualDensity: VisualDensity(horizontal: -4, vertical: -4),
          value: '',
          groupValue: groupValue.value,
          onChanged: onRadioChanged,
        ).marginOnly(right: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              InkWell(
                hoverColor: Colors.transparent,
                onTap: () => groupValue.value = '',
                child: Text(
                  translate('Ask the remote user for authentication'),
                ),
              ).marginOnly(bottom: 10),
              Text(
                translate('Choose this if the remote account is administrator'),
                style: TextStyle(fontSize: fontSizeNote),
              ),
            ],
          ).marginOnly(top: 3),
        ),
      ],
    ),
  );

  Widget OptionCredentials = Obx(
    () => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Radio(
          visualDensity: VisualDensity(horizontal: -4, vertical: -4),
          value: 'logon',
          groupValue: groupValue.value,
          onChanged: onRadioChanged,
        ).marginOnly(right: 10),
        Expanded(
          child: InkWell(
            hoverColor: Colors.transparent,
            onTap: () => onRadioChanged('logon'),
            child: Text(
              translate('Transmit the username and password of administrator'),
            ),
          ).marginOnly(top: 4),
        ),
      ],
    ),
  );

  Widget UacNote = Container(
    padding: EdgeInsets.fromLTRB(10, 8, 8, 8),
    decoration: BoxDecoration(
      color: MyTheme.currentThemeMode() == ThemeMode.dark
          ? Color.fromARGB(135, 87, 87, 90)
          : Colors.grey[100],
      borderRadius: BorderRadius.circular(8),
      border: Border.all(color: Colors.grey),
    ),
    child: Row(
      children: [
        Icon(Icons.info_outline_rounded, size: 20).marginOnly(right: 10),
        Expanded(
          child: Text(
            translate('still_click_uac_tip'),
            style: TextStyle(
                fontSize: fontSizeNote, fontWeight: FontWeight.normal),
          ),
        )
      ],
    ),
  );

  var content = Obx(
    () => Column(
      children: [
        OptionRequestPermissions.marginOnly(bottom: 15),
        OptionCredentials,
        Offstage(
          offstage: 'logon' != groupValue.value,
          child: Column(
            children: [
              UacNote.marginOnly(bottom: 10),
              DialogTextField(
                controller: userController,
                title: translate('Username'),
                hintText: translate('eg: admin'),
                prefixIcon: DialogTextField.kUsernameIcon,
                errorText: errUser.isEmpty ? null : errUser.value,
              ),
              PasswordWidget(
                controller: pwdController,
                autoFocus: false,
                errorText: errPwd.isEmpty ? null : errPwd.value,
              ),
            ],
          ).marginOnly(left: stateGlobal.isPortrait.isFalse ? 35 : 0),
        ).marginOnly(top: 10),
      ],
    ),
  );

  dialogManager.dismissAll();
  dialogManager.show(tag: '$sessionId-request-elevation',
      (setState, close, context) {
    void submit() {
      if (groupValue.value == 'logon') {
        if (userController.text.isEmpty) {
          errUser.value = translate('Empty Username');
          return;
        }
        if (pwdController.text.isEmpty) {
          errPwd.value = translate('Empty Password');
          return;
        }
        bind.sessionElevateWithLogon(
            sessionId: sessionId,
            username: userController.text,
            password: pwdController.text);
      } else {
        bind.sessionElevateDirect(sessionId: sessionId);
      }
      close();
      showWaitUacDialog(sessionId, dialogManager, "wait-uac");
    }

    return CustomAlertDialog(
      title: Text(translate('Request Elevation')),
      content: content,
      actions: [
        dialogButton(
          'Cancel',
          icon: Icon(Icons.close_rounded),
          onPressed: close,
          isOutline: true,
        ),
        dialogButton(
          'OK',
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        )
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showOnBlockDialog(
  SessionID sessionId,
  String type,
  String title,
  String text,
  OverlayDialogManager dialogManager,
) {
  if (dialogManager.existing('$sessionId-wait-uac') ||
      dialogManager.existing('$sessionId-request-elevation')) {
    return;
  }
  dialogManager.show(tag: '$sessionId-$type', (setState, close, context) {
    void submit() {
      close();
      showRequestElevationDialog(sessionId, dialogManager);
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title,
          "${translate(text)}${type.contains('uac') ? '\n' : '\n\n'}${translate('request_elevation_tip')}"),
      actions: [
        dialogButton('Wait', onPressed: close, isOutline: true),
        dialogButton('Request Elevation', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showElevationError(SessionID sessionId, String type, String title,
    String text, OverlayDialogManager dialogManager) {
  dialogManager.show(tag: '$sessionId-$type', (setState, close, context) {
    void submit() {
      close();
      showRequestElevationDialog(sessionId, dialogManager);
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title, text),
      actions: [
        dialogButton('Cancel', onPressed: () {
          close();
        }, isOutline: true),
        if (text != 'No permission') dialogButton('Retry', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showWaitAcceptDialog(SessionID sessionId, String type, String title,
    String text, OverlayDialogManager dialogManager) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    onCancel() {
      closeConnection();
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title, text),
      actions: [
        dialogButton('Cancel', onPressed: onCancel, isOutline: true),
      ],
      onCancel: onCancel,
    );
  });
}

void showRestartRemoteDevice(PeerInfo pi, String id, SessionID sessionId,
    OverlayDialogManager dialogManager) async {
  final res = await dialogManager
      .show<bool>((setState, close, context) => CustomAlertDialog(
            title: Row(children: [
              Icon(Icons.warning_rounded, color: Colors.redAccent, size: 28),
              Flexible(
                  child: Text(translate("Restart remote device"))
                      .paddingOnly(left: 10)),
            ]),
            content: Text(
                "${translate('Are you sure you want to restart')} \n${pi.username}@${pi.hostname}($id) ?"),
            actions: [
              dialogButton(
                "Cancel",
                icon: Icon(Icons.close_rounded),
                onPressed: close,
                isOutline: true,
              ),
              dialogButton(
                "OK",
                icon: Icon(Icons.done_rounded),
                onPressed: () => close(true),
              ),
            ],
            onCancel: close,
            onSubmit: () => close(true),
          ));
  if (res == true) bind.sessionRestartRemoteDevice(sessionId: sessionId);
}

showSetOSPassword(
  SessionID sessionId,
  bool login,
  OverlayDialogManager dialogManager,
  String? osPassword,
  Function()? closeCallback,
) async {
  final controller = TextEditingController();
  osPassword ??=
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-password') ??
          '';
  var autoLogin =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'auto-login') !=
          '';
  controller.text = osPassword;
  dialogManager.show((setState, close, context) {
    closeWithCallback([dynamic]) {
      close();
      if (closeCallback != null) closeCallback();
    }

    submit() {
      var text = controller.text.trim();
      bind.sessionPeerOption(
          sessionId: sessionId, name: 'os-password', value: text);
      bind.sessionPeerOption(
          sessionId: sessionId,
          name: 'auto-login',
          value: autoLogin ? 'Y' : '');
      if (text != '' && login) {
        bind.sessionInputOsPassword(sessionId: sessionId, value: text);
      }
      closeWithCallback();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.password_rounded, color: MyTheme.accent),
          Text(translate('OS Password')).paddingOnly(left: 10),
        ],
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          PasswordWidget(controller: controller),
          CheckboxListTile(
            contentPadding: const EdgeInsets.all(0),
            dense: true,
            controlAffinity: ListTileControlAffinity.leading,
            title: Text(
              translate('Auto Login'),
            ),
            value: autoLogin,
            onChanged: (v) {
              if (v == null) return;
              setState(() => autoLogin = v);
            },
          ),
        ],
      ),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: closeWithCallback,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: closeWithCallback,
    );
  });
}

showSetOSAccount(
  SessionID sessionId,
  OverlayDialogManager dialogManager,
) async {
  final usernameController = TextEditingController();
  final passwdController = TextEditingController();
  var username =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-username') ??
          '';
  var password =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-password') ??
          '';
  usernameController.text = username;
  passwdController.text = password;
  dialogManager.show((setState, close, context) {
    submit() {
      final username = usernameController.text.trim();
      final password = usernameController.text.trim();
      bind.sessionPeerOption(
          sessionId: sessionId, name: 'os-username', value: username);
      bind.sessionPeerOption(
          sessionId: sessionId, name: 'os-password', value: password);
      close();
    }

    descWidget(String text) {
      return Column(
        children: [
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              text,
              maxLines: 3,
              softWrap: true,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 16),
            ),
          ),
          Container(
            height: 8,
          ),
        ],
      );
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.password_rounded, color: MyTheme.accent),
          Text(translate('OS Account')).paddingOnly(left: 10),
        ],
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          descWidget(translate("os_account_desk_tip")),
          DialogTextField(
            title: translate(DialogTextField.kUsernameTitle),
            controller: usernameController,
            prefixIcon: DialogTextField.kUsernameIcon,
            errorText: null,
          ),
          PasswordWidget(controller: passwdController),
        ],
      ),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: close,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

showAuditDialog(FFI ffi) async {
  final controller = TextEditingController(text: ffi.auditNote);
  ffi.dialogManager.show((setState, close, context) {
    submit() {
      var text = controller.text;
      bind.sessionSendNote(sessionId: ffi.sessionId, note: text);
      ffi.auditNote = text;
      close();
    }

    late final focusNode = FocusNode(
      onKey: (FocusNode node, RawKeyEvent evt) {
        if (evt.logicalKey.keyLabel == 'Enter') {
          if (evt is RawKeyDownEvent) {
            int pos = controller.selection.base.offset;
            controller.text =
                '${controller.text.substring(0, pos)}\n${controller.text.substring(pos)}';
            controller.selection =
                TextSelection.fromPosition(TextPosition(offset: pos + 1));
          }
          return KeyEventResult.handled;
        }
        if (evt.logicalKey.keyLabel == 'Esc') {
          if (evt is RawKeyDownEvent) {
            close();
          }
          return KeyEventResult.handled;
        } else {
          return KeyEventResult.ignored;
        }
      },
    );

    return CustomAlertDialog(
      title: Text(translate('Note')),
      content: SizedBox(
          width: 250,
          height: 120,
          child: TextField(
            autofocus: true,
            keyboardType: TextInputType.multiline,
            textInputAction: TextInputAction.newline,
            decoration: const InputDecoration.collapsed(
              hintText: 'input note here',
            ),
            maxLines: null,
            maxLength: 256,
            controller: controller,
            focusNode: focusNode,
          ).workaroundFreezeLinuxMint()),
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit)
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showConfirmSwitchSidesDialog(
    SessionID sessionId, String id, OverlayDialogManager dialogManager) async {
  dialogManager.show((setState, close, context) {
    submit() async {
      await bind.sessionSwitchSides(sessionId: sessionId);
      closeConnection(id: id);
    }

    return CustomAlertDialog(
      content: msgboxContent('info', 'Switch Sides',
          'Please confirm if you want to share your desktop?'),
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

customImageQualityDialog(SessionID sessionId, String id, FFI ffi) async {
  double initQuality = kDefaultQuality;
  double initFps = kDefaultFps;
  bool qualitySet = false;
  bool fpsSet = false;

  bool? direct;
  try {
    direct =
        ConnectionTypeState.find(id).direct.value == ConnectionType.strDirect;
  } catch (_) {}
  bool hideFps = (await bind.mainIsUsingPublicServer() && direct != true) ||
      versionCmp(ffi.ffiModel.pi.version, '1.2.0') < 0;
  bool hideMoreQuality =
      (await bind.mainIsUsingPublicServer() && direct != true) ||
          versionCmp(ffi.ffiModel.pi.version, '1.2.2') < 0;

  setCustomValues({double? quality, double? fps}) async {
    debugPrint("setCustomValues quality:$quality, fps:$fps");
    if (quality != null) {
      qualitySet = true;
      await bind.sessionSetCustomImageQuality(
          sessionId: sessionId, value: quality.toInt());
    }
    if (fps != null) {
      fpsSet = true;
      await bind.sessionSetCustomFps(sessionId: sessionId, fps: fps.toInt());
    }
    if (!qualitySet) {
      qualitySet = true;
      await bind.sessionSetCustomImageQuality(
          sessionId: sessionId, value: initQuality.toInt());
    }
    if (!hideFps && !fpsSet) {
      fpsSet = true;
      await bind.sessionSetCustomFps(
          sessionId: sessionId, fps: initFps.toInt());
    }
  }

  final btnClose = dialogButton('Close', onPressed: () async {
    await setCustomValues();
    ffi.dialogManager.dismissAll();
  });

  // quality
  final quality = await bind.sessionGetCustomImageQuality(sessionId: sessionId);
  initQuality = quality != null && quality.isNotEmpty
      ? quality[0].toDouble()
      : kDefaultQuality;
  if (initQuality < kMinQuality ||
      initQuality > (!hideMoreQuality ? kMaxMoreQuality : kMaxQuality)) {
    initQuality = kDefaultQuality;
  }
  // fps
  final fpsOption =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'custom-fps');
  initFps = fpsOption == null
      ? kDefaultFps
      : double.tryParse(fpsOption) ?? kDefaultFps;
  if (initFps < kMinFps || initFps > kMaxFps) {
    initFps = kDefaultFps;
  }

  final content = customImageQualityWidget(
      initQuality: initQuality,
      initFps: initFps,
      setQuality: (v) => setCustomValues(quality: v),
      setFps: (v) => setCustomValues(fps: v),
      showFps: !hideFps,
      showMoreQuality: !hideMoreQuality);
  msgBoxCommon(ffi.dialogManager, 'Custom Image Quality', content, [btnClose]);
}

trackpadSpeedDialog(SessionID sessionId, FFI ffi) async {
  int initSpeed = ffi.inputModel.trackpadSpeed;
  final curSpeed = SimpleWrapper(initSpeed);
  final btnClose = dialogButton('Close', onPressed: () async {
    if (curSpeed.value <= kMaxTrackpadSpeed &&
        curSpeed.value >= kMinTrackpadSpeed &&
        curSpeed.value != initSpeed) {
      await bind.sessionSetTrackpadSpeed(
          sessionId: sessionId, value: curSpeed.value);
      await ffi.inputModel.updateTrackpadSpeed();
    }
    ffi.dialogManager.dismissAll();
  });
  msgBoxCommon(
      ffi.dialogManager,
      'Trackpad speed',
      TrackpadSpeedWidget(
        value: curSpeed,
      ),
      [btnClose]);
}

void deleteConfirmDialog(Function onSubmit, String title) async {
  gFFI.dialogManager.show(
    (setState, close, context) {
      submit() async {
        await onSubmit();
        close();
      }

      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(
              Icons.delete_rounded,
              color: Colors.red,
            ),
            Expanded(
              child: Text(title, overflow: TextOverflow.ellipsis).paddingOnly(
                left: 10,
              ),
            ),
          ],
        ),
        content: SizedBox.shrink(),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: close,
            isOutline: true,
          ),
          dialogButton(
            "OK",
            icon: Icon(Icons.done_rounded),
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    },
  );
}

void editAbTagDialog(
    List<dynamic> currentTags, Function(List<dynamic>) onSubmit) {
  var isInProgress = false;

  final tags = List.of(gFFI.abModel.currentAbTags);
  var selectedTag = currentTags.obs;

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      setState(() {
        isInProgress = true;
      });
      await onSubmit(selectedTag);
      close();
    }

    return CustomAlertDialog(
      title: Text(translate("Edit Tag")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: Wrap(
              children: tags
                  .map((e) => AddressBookTag(
                      name: e,
                      tags: selectedTag,
                      onTap: () {
                        if (selectedTag.contains(e)) {
                          selectedTag.remove(e);
                        } else {
                          selectedTag.add(e);
                        }
                      },
                      showActionMenu: false))
                  .toList(growable: false),
            ),
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void renameDialog(
    {required String oldName,
    FormFieldValidator<String>? validator,
    required ValueChanged<String> onSubmit,
    Function? onCancel}) async {
  RxBool isInProgress = false.obs;
  var controller = TextEditingController(text: oldName);
  final formKey = GlobalKey<FormState>();
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      String text = controller.text.trim();
      if (validator != null && formKey.currentState?.validate() == false) {
        return;
      }
      isInProgress.value = true;
      onSubmit(text);
      close();
      isInProgress.value = false;
    }

    cancel() {
      onCancel?.call();
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.edit_rounded, color: MyTheme.accent),
          Text(translate('Rename')).paddingOnly(left: 10),
        ],
      ),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            child: Form(
              key: formKey,
              child: TextFormField(
                controller: controller,
                autofocus: true,
                decoration: InputDecoration(labelText: translate('Name')),
                validator: validator,
              ).workaroundFreezeLinuxMint(),
            ),
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          Obx(() =>
              isInProgress.value ? const LinearProgressIndicator() : Offstage())
        ],
      ),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}

void changeBot({Function()? callback}) async {
  if (bind.mainHasValidBotSync()) {
    await bind.mainSetOption(key: "bot", value: "");
    callback?.call();
    return;
  }
  String errorText = '';
  bool loading = false;
  final controller = TextEditingController();
  gFFI.dialogManager.show((setState, close, context) {
    onVerify() async {
      final token = controller.text.trim();
      if (token == "") return;
      loading = true;
      errorText = '';
      setState(() {});
      final error = await bind.mainVerifyBot(token: token);
      if (error == "") {
        callback?.call();
        close();
      } else {
        errorText = translate(error);
        loading = false;
        setState(() {});
      }
    }

    final codeField = TextField(
      autofocus: true,
      controller: controller,
      decoration: InputDecoration(
        hintText: translate('Token'),
      ),
    ).workaroundFreezeLinuxMint();

    return CustomAlertDialog(
      title: Text(translate("Telegram bot")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SelectableText(translate("enable-bot-desc"),
                  style: TextStyle(fontSize: 12))
              .marginOnly(bottom: 12),
          Row(children: [Expanded(child: codeField)]),
          if (errorText != '')
            Text(errorText, style: TextStyle(color: Colors.red))
                .marginOnly(top: 12),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        loading
            ? CircularProgressIndicator()
            : dialogButton("OK", onPressed: onVerify),
      ],
      onCancel: close,
    );
  });
}

void change2fa({Function()? callback}) async {
  if (bind.mainHasValid2FaSync()) {
    await bind.mainSetOption(key: "2fa", value: "");
    await bind.mainClearTrustedDevices();
    callback?.call();
    return;
  }
  var new2fa = (await bind.mainGenerate2Fa());
  final secretRegex = RegExp(r'secret=([^&]+)');
  final secret = secretRegex.firstMatch(new2fa)?.group(1);
  String? errorText;
  final controller = TextEditingController();
  gFFI.dialogManager.show((setState, close, context) {
    onVerify() async {
      if (await bind.mainVerify2Fa(code: controller.text.trim())) {
        callback?.call();
        close();
      } else {
        errorText = translate('wrong-2fa-code');
      }
    }

    final codeField = Dialog2FaField(
      controller: controller,
      errorText: errorText,
      onChanged: () => setState(() => errorText = null),
      title: translate('Verification code'),
      readyCallback: () {
        onVerify();
        setState(() {});
      },
    );

    getOnSubmit() => codeField.isReady ? onVerify : null;

    return CustomAlertDialog(
      title: Text(translate("enable-2fa-title")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SelectableText(translate("enable-2fa-desc"),
                  style: TextStyle(fontSize: 12))
              .marginOnly(bottom: 12),
          SizedBox(
              width: 160,
              height: 160,
              child: QrImageView(
                backgroundColor: Colors.white,
                data: new2fa,
                version: QrVersions.auto,
                size: 160,
                gapless: false,
              )).marginOnly(bottom: 6),
          SelectableText(secret ?? '', style: TextStyle(fontSize: 12))
              .marginOnly(bottom: 12),
          Row(children: [Expanded(child: codeField)]),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: getOnSubmit()),
      ],
      onCancel: close,
    );
  });
}

void enter2FaDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  final controller = TextEditingController();
  final RxBool submitReady = false.obs;
  final RxBool trustThisDevice = false.obs;

  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      gFFI.send2FA(sessionId, controller.text.trim(), trustThisDevice.value);
      close();
      dialogManager.showLoading(translate('Logging in...'),
          onCancel: closeConnection);
    }

    late Dialog2FaField codeField;

    codeField = Dialog2FaField(
      controller: controller,
      title: translate('Verification code'),
      onChanged: () => submitReady.value = codeField.isReady,
    );

    final trustField = Obx(() => CheckboxListTile(
          contentPadding: const EdgeInsets.all(0),
          dense: true,
          controlAffinity: ListTileControlAffinity.leading,
          title: Text(translate("Trust this device")),
          value: trustThisDevice.value,
          onChanged: (value) {
            if (value == null) return;
            trustThisDevice.value = value;
          },
        ));

    return CustomAlertDialog(
        title: Text(translate('enter-2fa-title')),
        content: Column(
          children: [
            codeField,
            if (bind.sessionGetEnableTrustedDevices(sessionId: sessionId))
              trustField,
          ],
        ),
        actions: [
          dialogButton('Cancel',
              onPressed: cancel,
              isOutline: true,
              style: TextStyle(
                  color: Theme.of(context).textTheme.bodyMedium?.color)),
          Obx(() => dialogButton(
                'OK',
                onPressed: submitReady.isTrue ? submit : null,
              )),
        ],
        onSubmit: submit,
        onCancel: cancel);
  });
}

// This dialog should not be dismissed, otherwise it will be black screen, have not reproduced this.
void showWindowsSessionsDialog(
    String type,
    String title,
    String text,
    OverlayDialogManager dialogManager,
    SessionID sessionId,
    String peerId,
    String sessions) {
  List<dynamic> sessionsList = [];
  try {
    sessionsList = json.decode(sessions);
  } catch (e) {
    print(e);
  }
  List<String> sids = [];
  List<String> names = [];
  for (var session in sessionsList) {
    sids.add(session['sid']);
    names.add(session['name']);
  }
  String selectedUserValue = sids.first;
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    submit() {
      bind.sessionSendSelectedSessionId(
          sessionId: sessionId, sid: selectedUserValue);
      close();
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title, text),
      actions: [
        ComboBox(
            keys: sids,
            values: names,
            initialKey: selectedUserValue,
            onChanged: (value) {
              selectedUserValue = value;
            }),
        dialogButton('Connect', onPressed: submit, isOutline: false),
      ],
    );
  });
}

void addPeersToAbDialog(
  List<Peer> peers,
) async {
  Future<bool> addTo(String abname) async {
    final mapList = peers.map((e) {
      var json = e.toJson();
      // remove password when add to another address book to avoid re-share
      json.remove('password');
      json.remove('hash');
      return json;
    }).toList();
    final errMsg = await gFFI.abModel.addPeersTo(mapList, abname);
    if (errMsg == null) {
      showToast(translate('Successful'));
      return true;
    } else {
      BotToast.showText(text: errMsg, contentColor: Colors.red);
      return false;
    }
  }

  // if only one address book and it is personal, add to it directly
  if (gFFI.abModel.addressbooks.length == 1 &&
      gFFI.abModel.current.isPersonal()) {
    await addTo(gFFI.abModel.currentName.value);
    return;
  }

  RxBool isInProgress = false.obs;
  final names = gFFI.abModel.addressBooksCanWrite();
  RxString currentName = gFFI.abModel.currentName.value.obs;
  TextEditingController controller = TextEditingController();
  if (gFFI.peerTabModel.currentTab == PeerTabIndex.ab.index) {
    names.remove(currentName.value);
  }
  if (names.isEmpty) {
    debugPrint('no address book to add peers to, should not happen');
    return;
  }
  if (!names.contains(currentName.value)) {
    currentName.value = names[0];
  }
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      if (controller.text != gFFI.abModel.translatedName(currentName.value)) {
        BotToast.showText(
            text: 'illegal address book name: ${controller.text}',
            contentColor: Colors.red);
        return;
      }
      isInProgress.value = true;
      if (await addTo(currentName.value)) {
        close();
      }
      isInProgress.value = false;
    }

    cancel() {
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(IconFont.addressBook, color: MyTheme.accent),
          Text(translate('Add to address book')).paddingOnly(left: 10),
        ],
      ),
      content: Obx(() => Column(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              // https://github.com/flutter/flutter/issues/145081
              DropdownMenu(
                initialSelection: currentName.value,
                onSelected: (value) {
                  if (value != null) {
                    currentName.value = value;
                  }
                },
                dropdownMenuEntries: names
                    .map((e) => DropdownMenuEntry(
                        value: e, label: gFFI.abModel.translatedName(e)))
                    .toList(),
                inputDecorationTheme: InputDecorationTheme(
                    isDense: true, border: UnderlineInputBorder()),
                enableFilter: true,
                controller: controller,
              ),
              // NOT use Offstage to wrap LinearProgressIndicator
              isInProgress.value ? const LinearProgressIndicator() : Offstage()
            ],
          )),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}

void setSharedAbPasswordDialog(String abName, Peer peer) {
  TextEditingController controller = TextEditingController(text: '');
  RxBool isInProgress = false.obs;
  RxBool isInputEmpty = true.obs;
  bool passwordVisible = false;
  controller.addListener(() {
    isInputEmpty.value = controller.text.isEmpty;
  });
  gFFI.dialogManager.show((setState, close, context) {
    change(String password) async {
      isInProgress.value = true;
      bool res =
          await gFFI.abModel.changeSharedPassword(abName, peer.id, password);
      isInProgress.value = false;
      if (res) {
        showToast(translate('Successful'));
      }
      close();
    }

    cancel() {
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.key, color: MyTheme.accent),
          Text(translate(peer.password.isEmpty
                  ? 'Set shared password'
                  : 'Change Password'))
              .paddingOnly(left: 10),
        ],
      ),
      content: Obx(() => Column(children: [
            TextField(
              controller: controller,
              autofocus: true,
              obscureText: !passwordVisible,
              decoration: InputDecoration(
                suffixIcon: IconButton(
                  icon: Icon(
                      passwordVisible ? Icons.visibility : Icons.visibility_off,
                      color: MyTheme.lightTheme.primaryColor),
                  onPressed: () {
                    setState(() {
                      passwordVisible = !passwordVisible;
                    });
                  },
                ),
              ),
            ).workaroundFreezeLinuxMint(),
            if (!gFFI.abModel.current.isPersonal())
              Row(children: [
                Icon(Icons.info, color: Colors.amber).marginOnly(right: 4),
                Text(
                  translate('share_warning_tip'),
                  style: TextStyle(fontSize: 12),
                )
              ]).marginSymmetric(vertical: 10),
            // NOT use Offstage to wrap LinearProgressIndicator
            isInProgress.value ? const LinearProgressIndicator() : Offstage()
          ])),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        if (peer.password.isNotEmpty)
          dialogButton(
            "Remove",
            icon: Icon(Icons.delete_outline_rounded),
            onPressed: () => change(''),
            buttonStyle: ButtonStyle(
                backgroundColor: MaterialStatePropertyAll(Colors.red)),
          ),
        Obx(() => dialogButton(
              "OK",
              icon: Icon(Icons.done_rounded),
              onPressed:
                  isInputEmpty.value ? null : () => change(controller.text),
            )),
      ],
      onSubmit: isInputEmpty.value ? null : () => change(controller.text),
      onCancel: cancel,
    );
  });
}

void CommonConfirmDialog(OverlayDialogManager dialogManager, String content,
    VoidCallback onConfirm) {
  dialogManager.show((setState, close, context) {
    submit() {
      close();
      onConfirm.call();
    }

    return CustomAlertDialog(
      content: Row(
        children: [
          Expanded(
            child: Text(content,
                style: const TextStyle(fontSize: 15),
                textAlign: TextAlign.start),
          ),
        ],
      ).marginOnly(bottom: 12),
      actions: [
        dialogButton(translate("Cancel"), onPressed: close, isOutline: true),
        dialogButton(translate("OK"), onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void changeUnlockPinDialog(String oldPin, Function() callback) {
  final pinController = TextEditingController(text: oldPin);
  final confirmController = TextEditingController(text: oldPin);
  String? pinErrorText;
  String? confirmationErrorText;
  final maxLength = bind.mainMaxEncryptLen();
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      pinErrorText = null;
      confirmationErrorText = null;
      final pin = pinController.text.trim();
      final confirm = confirmController.text.trim();
      if (pin != confirm) {
        setState(() {
          confirmationErrorText =
              translate('The confirmation is not identical.');
        });
        return;
      }
      final errorMsg = bind.mainSetUnlockPin(pin: pin);
      if (errorMsg != '') {
        setState(() {
          pinErrorText = translate(errorMsg);
        });
        return;
      }
      callback.call();
      close();
    }

    return CustomAlertDialog(
      title: Text(translate("Set PIN")),
      content: Column(
        children: [
          DialogTextField(
            title: 'PIN',
            controller: pinController,
            obscureText: true,
            errorText: pinErrorText,
            maxLength: maxLength,
          ),
          DialogTextField(
            title: translate('Confirmation'),
            controller: confirmController,
            obscureText: true,
            errorText: confirmationErrorText,
            maxLength: maxLength,
          )
        ],
      ).marginOnly(bottom: 12),
      actions: [
        dialogButton(translate("Cancel"), onPressed: close, isOutline: true),
        dialogButton(translate("OK"), onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void checkUnlockPinDialog(String correctPin, Function() passCallback) {
  final controller = TextEditingController();
  String? errorText;
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      final pin = controller.text.trim();
      if (correctPin != pin) {
        setState(() {
          errorText = translate('Wrong PIN');
        });
        return;
      }
      passCallback.call();
      close();
    }

    return CustomAlertDialog(
      content: Row(
        children: [
          Expanded(
              child: PasswordWidget(
            title: 'PIN',
            controller: controller,
            errorText: errorText,
            hintText: '',
          ))
        ],
      ).marginOnly(bottom: 12),
      actions: [
        dialogButton(translate("Cancel"), onPressed: close, isOutline: true),
        dialogButton(translate("OK"), onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void confrimDeleteTrustedDevicesDialog(
    RxList<TrustedDevice> trustedDevices, RxList<Uint8List> selectedDevices) {
  CommonConfirmDialog(gFFI.dialogManager, '${translate('Confirm Delete')}?',
      () async {
    if (selectedDevices.isEmpty) return;
    if (selectedDevices.length == trustedDevices.length) {
      await bind.mainClearTrustedDevices();
      trustedDevices.clear();
      selectedDevices.clear();
    } else {
      final json = jsonEncode(selectedDevices.map((e) => e.toList()).toList());
      await bind.mainRemoveTrustedDevices(json: json);
      trustedDevices.removeWhere((element) {
        return selectedDevices.contains(element.hwid);
      });
      selectedDevices.clear();
    }
  });
}

void manageTrustedDeviceDialog() async {
  RxList<TrustedDevice> trustedDevices = (await TrustedDevice.get()).obs;
  RxList<Uint8List> selectedDevices = RxList.empty();
  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Manage trusted devices")),
      content: trustedDevicesTable(trustedDevices, selectedDevices),
      actions: [
        Obx(() => dialogButton(translate("Delete"),
                onPressed: selectedDevices.isEmpty
                    ? null
                    : () {
                        confrimDeleteTrustedDevicesDialog(
                          trustedDevices,
                          selectedDevices,
                        );
                      },
                isOutline: false)
            .marginOnly(top: 12)),
        dialogButton(translate("Close"), onPressed: close, isOutline: true)
            .marginOnly(top: 12),
      ],
      onCancel: close,
    );
  });
}

class TrustedDevice {
  late final Uint8List hwid;
  late final int time;
  late final String id;
  late final String name;
  late final String platform;

  TrustedDevice.fromJson(Map<String, dynamic> json) {
    final hwidList = json['hwid'] as List<dynamic>;
    hwid = Uint8List.fromList(hwidList.cast<int>());
    time = json['time'];
    id = json['id'];
    name = json['name'];
    platform = json['platform'];
  }

  String daysRemaining() {
    final expiry = time + 90 * 24 * 60 * 60 * 1000;
    final remaining = expiry - DateTime.now().millisecondsSinceEpoch;
    if (remaining < 0) {
      return '0';
    }
    return (remaining / (24 * 60 * 60 * 1000)).toStringAsFixed(0);
  }

  static Future<List<TrustedDevice>> get() async {
    final List<TrustedDevice> devices = List.empty(growable: true);
    try {
      final devicesJson = await bind.mainGetTrustedDevices();
      if (devicesJson.isNotEmpty) {
        final devicesList = json.decode(devicesJson);
        if (devicesList is List) {
          for (var device in devicesList) {
            devices.add(TrustedDevice.fromJson(device));
          }
        }
      }
    } catch (e) {
      print(e.toString());
    }
    devices.sort((a, b) => b.time.compareTo(a.time));
    return devices;
  }
}

Widget trustedDevicesTable(
    RxList<TrustedDevice> devices, RxList<Uint8List> selectedDevices) {
  RxBool selectAll = false.obs;
  setSelectAll() {
    if (selectedDevices.isNotEmpty &&
        selectedDevices.length == devices.length) {
      selectAll.value = true;
    } else {
      selectAll.value = false;
    }
  }

  devices.listen((_) {
    setSelectAll();
  });
  selectedDevices.listen((_) {
    setSelectAll();
  });
  return FittedBox(
    child: Obx(() => DataTable(
          columns: [
            DataColumn(
                label: Checkbox(
              value: selectAll.value,
              onChanged: (value) {
                if (value == true) {
                  selectedDevices.clear();
                  selectedDevices.addAll(devices.map((e) => e.hwid));
                } else {
                  selectedDevices.clear();
                }
              },
            )),
            DataColumn(label: Text(translate('Platform'))),
            DataColumn(label: Text(translate('ID'))),
            DataColumn(label: Text(translate('Username'))),
            DataColumn(label: Text(translate('Days remaining'))),
          ],
          rows: devices.map((device) {
            return DataRow(cells: [
              DataCell(Checkbox(
                value: selectedDevices.contains(device.hwid),
                onChanged: (value) {
                  if (value == null) return;
                  if (value) {
                    selectedDevices.remove(device.hwid);
                    selectedDevices.add(device.hwid);
                  } else {
                    selectedDevices.remove(device.hwid);
                  }
                },
              )),
              DataCell(Text(device.platform)),
              DataCell(Text(device.id)),
              DataCell(Text(device.name)),
              DataCell(Text(device.daysRemaining())),
            ]);
          }).toList(),
        )),
  );
}
