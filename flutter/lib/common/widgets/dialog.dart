import 'dart:async';

import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';

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
    RegexValidationRule('allowed characters', RegExp(r'^\w*$'))
  ];

  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      debugPrint("onSubmit");
      newId = controller.text.trim();

      final Iterable violations = rules.where((r) => !r.validate(newId));
      if (violations.isNotEmpty) {
        setState(() {
          msg = isDesktop
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
        msg = isDesktop
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
          ),
          const SizedBox(
            height: 8.0,
          ),
          isDesktop
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
          Offstage(
              offstage: !isInProgress, child: const LinearProgressIndicator())
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
  var newWhiteList = (await bind.mainGetOption(key: 'whitelist')).split(',');
  var newWhiteListField = newWhiteList.join('\n');
  var controller = TextEditingController(text: newWhiteListField);
  var msg = "";
  var isInProgress = false;
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
                    autofocus: true),
              ),
            ],
          ),
          const SizedBox(
            height: 4.0,
          ),
          Offstage(
              offstage: !isInProgress, child: const LinearProgressIndicator())
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("Clear", onPressed: () async {
          await bind.mainSetOption(key: 'whitelist', value: '');
          callback?.call();
          close();
        }, isOutline: true),
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
              final ips = newWhiteListField.trim().split(RegExp(r"[\s,;\n]+"));
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
            await bind.mainSetOption(key: 'whitelist', value: newWhiteList);
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
                    autofocus: true),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () async {
          await bind.mainSetOption(
              key: 'direct-access-port', value: controller.text);
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
      required this.title,
      required this.controller})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: TextField(
            decoration: InputDecoration(
              labelText: title,
              hintText: hintText,
              prefixIcon: prefixIcon,
              suffixIcon: suffixIcon,
              helperText: helperText,
              helperMaxLines: 8,
              errorText: errorText,
              errorMaxLines: 8,
            ),
            controller: controller,
            focusNode: focusNode,
            autofocus: true,
            obscureText: obscureText,
          ),
        ),
      ],
    ).paddingSymmetric(vertical: 4.0);
  }
}

class PasswordWidget extends StatefulWidget {
  PasswordWidget({
    Key? key,
    required this.controller,
    this.autoFocus = true,
    this.hintText,
    this.errorText,
  }) : super(key: key);

  final TextEditingController controller;
  final bool autoFocus;
  final String? hintText;
  final String? errorText;

  @override
  State<PasswordWidget> createState() => _PasswordWidgetState();
}

class _PasswordWidgetState extends State<PasswordWidget> {
  bool _passwordVisible = false;
  final _focusNode = FocusNode();
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    if (widget.autoFocus) {
      _timer =
          Timer(Duration(milliseconds: 50), () => _focusNode.requestFocus());
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    _focusNode.unfocus();
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return DialogTextField(
      title: translate(DialogTextField.kPasswordTitle),
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
          ).marginOnly(left: isDesktop ? 35 : 0),
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
        dialogButton('Retry', onPressed: submit),
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
                  child: Text(translate("Restart Remote Device"))
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
) async {
  final controller = TextEditingController();
  var password =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-password') ??
          '';
  var autoLogin =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'auto-login') !=
          '';
  controller.text = password;
  dialogManager.show((setState, close, context) {
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
      close();
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

showAuditDialog(SessionID sessionId, dialogManager) async {
  final controller = TextEditingController();
  dialogManager.show((setState, close, context) {
    submit() {
      var text = controller.text.trim();
      if (text != '') {
        bind.sessionSendNote(sessionId: sessionId, note: text);
      }
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
          )),
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
  double qualityInitValue = 50;
  double fpsInitValue = 30;
  bool qualitySet = false;
  bool fpsSet = false;
  setCustomValues({double? quality, double? fps}) async {
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
          sessionId: sessionId, value: qualityInitValue.toInt());
    }
    if (!fpsSet) {
      fpsSet = true;
      await bind.sessionSetCustomFps(
          sessionId: sessionId, fps: fpsInitValue.toInt());
    }
  }

  final btnClose = dialogButton('Close', onPressed: () async {
    await setCustomValues();
    ffi.dialogManager.dismissAll();
  });

  // quality
  final quality = await bind.sessionGetCustomImageQuality(sessionId: sessionId);
  qualityInitValue =
      quality != null && quality.isNotEmpty ? quality[0].toDouble() : 50.0;
  const qualityMinValue = 10.0;
  const qualityMaxValue = 100.0;
  if (qualityInitValue < qualityMinValue) {
    qualityInitValue = qualityMinValue;
  }
  if (qualityInitValue > qualityMaxValue) {
    qualityInitValue = qualityMaxValue;
  }
  final RxDouble qualitySliderValue = RxDouble(qualityInitValue);
  final debouncerQuality = Debouncer<double>(
    Duration(milliseconds: 1000),
    onChanged: (double v) {
      setCustomValues(quality: v);
    },
    initialValue: qualityInitValue,
  );
  final qualitySlider = Obx(() => Row(
        children: [
          Expanded(
              flex: 3,
              child: Slider(
                value: qualitySliderValue.value,
                min: qualityMinValue,
                max: qualityMaxValue,
                divisions: 18,
                onChanged: (double value) {
                  qualitySliderValue.value = value;
                  debouncerQuality.value = value;
                },
              )),
          Expanded(
              flex: 1,
              child: Text(
                '${qualitySliderValue.value.round()}%',
                style: const TextStyle(fontSize: 15),
              )),
          Expanded(
              flex: 2,
              child: Text(
                translate('Bitrate'),
                style: const TextStyle(fontSize: 15),
              )),
        ],
      ));
  // fps
  final fpsOption =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'custom-fps');
  fpsInitValue = fpsOption == null ? 30 : double.tryParse(fpsOption) ?? 30;
  if (fpsInitValue < 5 || fpsInitValue > 120) {
    fpsInitValue = 30;
  }
  final RxDouble fpsSliderValue = RxDouble(fpsInitValue);
  final debouncerFps = Debouncer<double>(
    Duration(milliseconds: 1000),
    onChanged: (double v) {
      setCustomValues(fps: v);
    },
    initialValue: qualityInitValue,
  );
  bool? direct;
  try {
    direct =
        ConnectionTypeState.find(id).direct.value == ConnectionType.strDirect;
  } catch (_) {}
  final fpsSlider = Offstage(
    offstage: (await bind.mainIsUsingPublicServer() && direct != true) ||
        version_cmp(ffi.ffiModel.pi.version, '1.2.0') < 0,
    child: Row(
      children: [
        Expanded(
            flex: 3,
            child: Obx((() => Slider(
                  value: fpsSliderValue.value,
                  min: 5,
                  max: 120,
                  divisions: 23,
                  onChanged: (double value) {
                    fpsSliderValue.value = value;
                    debouncerFps.value = value;
                  },
                )))),
        Expanded(
            flex: 1,
            child: Obx(() => Text(
                  '${fpsSliderValue.value.round()}',
                  style: const TextStyle(fontSize: 15),
                ))),
        Expanded(
            flex: 2,
            child: Text(
              translate('FPS'),
              style: const TextStyle(fontSize: 15),
            ))
      ],
    ),
  );

  final content = Column(
    children: [qualitySlider, fpsSlider],
  );
  msgBoxCommon(ffi.dialogManager, 'Custom Image Quality', content, [btnClose]);
}
