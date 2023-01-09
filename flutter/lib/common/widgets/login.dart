import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/hbbs/hbbs.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';

class _IconOP extends StatelessWidget {
  final String icon;
  final double iconWidth;
  final EdgeInsets margin;
  const _IconOP(
      {Key? key,
      required this.icon,
      required this.iconWidth,
      this.margin = const EdgeInsets.symmetric(horizontal: 4.0)})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: margin,
      child: SvgPicture.asset(
        'assets/$icon.svg',
        width: iconWidth,
      ),
    );
  }
}

class ButtonOP extends StatelessWidget {
  final String op;
  final RxString curOP;
  final double iconWidth;
  final Color primaryColor;
  final double height;
  final Function() onTap;

  const ButtonOP({
    Key? key,
    required this.op,
    required this.curOP,
    required this.iconWidth,
    required this.primaryColor,
    required this.height,
    required this.onTap,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Row(children: [
      Container(
        height: height,
        width: 200,
        child: Obx(() => ElevatedButton(
            style: ElevatedButton.styleFrom(
              primary: curOP.value.isEmpty || curOP.value == op
                  ? primaryColor
                  : Colors.grey,
            ).copyWith(elevation: ButtonStyleButton.allOrNull(0.0)),
            onPressed: curOP.value.isEmpty || curOP.value == op ? onTap : null,
            child: Row(
              children: [
                SizedBox(
                    width: 30,
                    child: _IconOP(
                      icon: op,
                      iconWidth: iconWidth,
                      margin: EdgeInsets.only(right: 5),
                    )),
                Expanded(
                    child: FittedBox(
                        fit: BoxFit.scaleDown,
                        child: Center(
                            child: Text('${translate("Continue with")} $op')))),
              ],
            ))),
      ),
    ]);
  }
}

class ConfigOP {
  final String op;
  final double iconWidth;
  ConfigOP({required this.op, required this.iconWidth});
}

class WidgetOP extends StatefulWidget {
  final ConfigOP config;
  final RxString curOP;
  final Function(String) cbLogin;
  const WidgetOP({
    Key? key,
    required this.config,
    required this.curOP,
    required this.cbLogin,
  }) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _WidgetOPState();
  }
}

class _WidgetOPState extends State<WidgetOP> {
  Timer? _updateTimer;
  String _stateMsg = '';
  String _failedMsg = '';
  String _url = '';

  @override
  void initState() {
    super.initState();
  }

  @override
  void dispose() {
    super.dispose();
    _updateTimer?.cancel();
  }

  _beginQueryState() {
    _updateTimer = Timer.periodic(Duration(seconds: 1), (timer) {
      _updateState();
    });
  }

  _updateState() {
    bind.mainAccountAuthResult().then((result) {
      if (result.isEmpty) {
        return;
      }
      final resultMap = jsonDecode(result);
      if (resultMap == null) {
        return;
      }
      final String stateMsg = resultMap['state_msg'];
      String failedMsg = resultMap['failed_msg'];
      final String? url = resultMap['url'];
      final authBody = resultMap['auth_body'];
      if (_stateMsg != stateMsg || _failedMsg != failedMsg) {
        if (_url.isEmpty && url != null && url.isNotEmpty) {
          launchUrl(Uri.parse(url));
          _url = url;
        }
        if (authBody != null) {
          _updateTimer?.cancel();
          final String username = authBody['user']['name'];
          widget.curOP.value = '';
          widget.cbLogin(username);
        }

        setState(() {
          _stateMsg = stateMsg;
          _failedMsg = failedMsg;
          if (failedMsg.isNotEmpty) {
            widget.curOP.value = '';
            _updateTimer?.cancel();
          }
        });
      }
    });
  }

  _resetState() {
    _stateMsg = '';
    _failedMsg = '';
    _url = '';
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ButtonOP(
          op: widget.config.op,
          curOP: widget.curOP,
          iconWidth: widget.config.iconWidth,
          primaryColor: str2color(widget.config.op, 0x7f),
          height: 36,
          onTap: () async {
            _resetState();
            widget.curOP.value = widget.config.op;
            await bind.mainAccountAuth(op: widget.config.op);
            _beginQueryState();
          },
        ),
        Obx(() {
          if (widget.curOP.isNotEmpty &&
              widget.curOP.value != widget.config.op) {
            _failedMsg = '';
          }
          return Offstage(
              offstage:
                  _failedMsg.isEmpty && widget.curOP.value != widget.config.op,
              child: Row(
                children: [
                  Text(
                    _stateMsg,
                    style: TextStyle(fontSize: 12),
                  ),
                  SizedBox(width: 8),
                  Text(
                    _failedMsg,
                    style: TextStyle(
                      fontSize: 14,
                      color: Colors.red,
                    ),
                  ),
                ],
              ));
        }),
        Obx(
          () => Offstage(
            offstage: widget.curOP.value != widget.config.op,
            child: const SizedBox(
              height: 5.0,
            ),
          ),
        ),
        Obx(
          () => Offstage(
            offstage: widget.curOP.value != widget.config.op,
            child: ConstrainedBox(
              constraints: BoxConstraints(maxHeight: 20),
              child: ElevatedButton(
                onPressed: () {
                  widget.curOP.value = '';
                  _updateTimer?.cancel();
                  _resetState();
                  bind.mainAccountAuthCancel();
                },
                child: Text(
                  translate('Cancel'),
                  style: TextStyle(fontSize: 15),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class LoginWidgetOP extends StatelessWidget {
  final List<ConfigOP> ops;
  final RxString curOP;
  final Function(String) cbLogin;

  LoginWidgetOP({
    Key? key,
    required this.ops,
    required this.curOP,
    required this.cbLogin,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var children = ops
        .map((op) => [
              WidgetOP(
                config: op,
                curOP: curOP,
                cbLogin: cbLogin,
              ),
              const Divider(
                indent: 5,
                endIndent: 5,
              )
            ])
        .expand((i) => i)
        .toList();
    if (children.isNotEmpty) {
      children.removeLast();
    }
    return SingleChildScrollView(
        child: Container(
            width: 200,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              mainAxisAlignment: MainAxisAlignment.spaceAround,
              children: children,
            )));
  }
}

class LoginWidgetUserPass extends StatelessWidget {
  final TextEditingController username;
  final TextEditingController pass;
  final String? usernameMsg;
  final String? passMsg;
  final bool isInProgress;
  final RxString curOP;
  final RxBool autoLogin;
  final Function() onLogin;
  final FocusNode? userFocusNode;
  const LoginWidgetUserPass({
    Key? key,
    this.userFocusNode,
    required this.username,
    required this.pass,
    required this.usernameMsg,
    required this.passMsg,
    required this.isInProgress,
    required this.curOP,
    required this.autoLogin,
    required this.onLogin,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Padding(
        padding: EdgeInsets.all(0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            const SizedBox(height: 8.0),
            DialogTextField(
                title: '${translate("Username")}:',
                controller: username,
                focusNode: userFocusNode,
                prefixIcon: Icon(Icons.account_circle_outlined),
                errorText: usernameMsg),
            DialogTextField(
                title: '${translate("Password")}:',
                obscureText: true,
                controller: pass,
                prefixIcon: Icon(Icons.lock_outline),
                errorText: passMsg),
            Obx(() => CheckboxListTile(
                  contentPadding: const EdgeInsets.all(0),
                  dense: true,
                  controlAffinity: ListTileControlAffinity.leading,
                  title: Text(
                    translate("Remember me"),
                  ),
                  value: autoLogin.value,
                  onChanged: (v) {
                    if (v == null) return;
                    autoLogin.value = v;
                  },
                )),
            Offstage(
                offstage: !isInProgress,
                child: const LinearProgressIndicator()),
            const SizedBox(height: 12.0),
            FittedBox(
                child:
                    Row(mainAxisAlignment: MainAxisAlignment.center, children: [
              Container(
                height: 38,
                width: 200,
                child: Obx(() => ElevatedButton(
                      child: Text(
                        translate('Login'),
                        style: TextStyle(fontSize: 16),
                      ),
                      onPressed:
                          curOP.value.isEmpty || curOP.value == 'rustdesk'
                              ? () {
                                  onLogin();
                                }
                              : null,
                    )),
              ),
            ])),
          ],
        ));
  }
}

class DialogTextField extends StatelessWidget {
  final String title;
  final bool obscureText;
  final String? errorText;
  final String? helperText;
  final Widget? prefixIcon;
  final TextEditingController controller;
  final FocusNode? focusNode;

  DialogTextField(
      {Key? key,
      this.focusNode,
      this.obscureText = false,
      this.errorText,
      this.helperText,
      this.prefixIcon,
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
                border: const OutlineInputBorder(),
                prefixIcon: prefixIcon,
                helperText: helperText,
                helperMaxLines: 8,
                errorText: errorText),
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

/// common login dialog for desktop
/// call this directly
Future<bool?> loginDialog() async {
  var username = TextEditingController();
  var password = TextEditingController();
  final userFocusNode = FocusNode()..requestFocus();
  Timer(Duration(milliseconds: 100), () => userFocusNode..requestFocus());

  String? usernameMsg;
  String? passwordMsg;
  var isInProgress = false;
  final autoLogin = true.obs;
  final RxString curOP = ''.obs;

  final res = await gFFI.dialogManager.show<bool>((setState, close) {
    username.addListener(() {
      if (usernameMsg != null) {
        setState(() => usernameMsg = null);
      }
    });

    password.addListener(() {
      if (passwordMsg != null) {
        setState(() => passwordMsg = null);
      }
    });

    onDialogCancel() {
      isInProgress = false;
      close(false);
    }

    onLogin() async {
      // validate
      if (username.text.isEmpty) {
        setState(() => usernameMsg = translate('Username missed'));
        return;
      }
      if (password.text.isEmpty) {
        setState(() => passwordMsg = translate('Password missed'));
        return;
      }
      curOP.value = 'rustdesk';
      setState(() => isInProgress = true);
      try {
        final resp = await gFFI.userModel.login(LoginRequest(
            username: username.text,
            password: password.text,
            id: await bind.mainGetMyId(),
            uuid: await bind.mainGetUuid(),
            autoLogin: autoLogin.value,
            type: HttpType.kAuthReqTypeAccount));

        switch (resp.type) {
          case HttpType.kAuthResTypeToken:
            if (resp.access_token != null) {
              await bind.mainSetLocalOption(
                  key: 'access_token', value: resp.access_token!);
              close(true);
              return;
            }
            break;
          case HttpType.kAuthResTypeEmailCheck:
            setState(() => isInProgress = false);
            final res = await verificationCodeDialog(resp.user);
            if (res == true) {
              close(true);
              return;
            }
            break;
          default:
            passwordMsg = "Failed, bad response from server";
            break;
        }
      } on RequestException catch (err) {
        passwordMsg = translate(err.cause);
        debugPrintStack(label: err.toString());
      } catch (err) {
        passwordMsg = "Unknown Error: $err";
        debugPrintStack(label: err.toString());
      }
      curOP.value = '';
      setState(() => isInProgress = false);
    }

    return CustomAlertDialog(
      title: Text(translate('Login')),
      contentBoxConstraints: BoxConstraints(minWidth: 400),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const SizedBox(
            height: 8.0,
          ),
          LoginWidgetUserPass(
            username: username,
            pass: password,
            usernameMsg: usernameMsg,
            passMsg: passwordMsg,
            isInProgress: isInProgress,
            curOP: curOP,
            autoLogin: autoLogin,
            onLogin: onLogin,
            userFocusNode: userFocusNode,
          ),
          const SizedBox(
            height: 8.0,
          ),
          Center(
              child: Text(
            translate('or'),
            style: TextStyle(fontSize: 16),
          )),
          const SizedBox(
            height: 8.0,
          ),
          LoginWidgetOP(
            ops: [
              ConfigOP(op: 'Github', iconWidth: 20),
              ConfigOP(op: 'Google', iconWidth: 20),
              ConfigOP(op: 'Okta', iconWidth: 38),
            ],
            curOP: curOP,
            cbLogin: (String username) {
              gFFI.userModel.userName.value = username;
              close(true);
            },
          ),
        ],
      ),
      actions: [msgBoxButton(translate('Close'), onDialogCancel)],
      onCancel: onDialogCancel,
    );
  });

  if (res != null) {
    // update ab and group status
    await gFFI.abModel.pullAb();
    await gFFI.groupModel.pull();
  }

  return res;
}

Future<bool?> verificationCodeDialog(UserPayload? user) async {
  var autoLogin = true;
  var isInProgress = false;
  String? errorText;

  final code = TextEditingController();
  final focusNode = FocusNode()..requestFocus();
  Timer(Duration(milliseconds: 100), () => focusNode..requestFocus());

  final res = await gFFI.dialogManager.show<bool>((setState, close) {
    bool validate() {
      return code.text.length >= 6;
    }

    code.addListener(() {
      if (errorText != null) {
        setState(() => errorText = null);
      }
    });

    void onVerify() async {
      if (!validate()) {
        setState(
            () => errorText = translate('Too short, at least 6 characters.'));
        return;
      }
      setState(() => isInProgress = true);

      try {
        final resp = await gFFI.userModel.login(LoginRequest(
            verificationCode: code.text,
            username: user?.name,
            id: await bind.mainGetMyId(),
            uuid: await bind.mainGetUuid(),
            autoLogin: autoLogin,
            type: HttpType.kAuthReqTypeEmailCode));

        switch (resp.type) {
          case HttpType.kAuthResTypeToken:
            if (resp.access_token != null) {
              await bind.mainSetLocalOption(
                  key: 'access_token', value: resp.access_token!);
              close(true);
              return;
            }
            break;
          default:
            errorText = "Failed, bad response from server";
            break;
        }
      } on RequestException catch (err) {
        errorText = translate(err.cause);
        debugPrintStack(label: err.toString());
      } catch (err) {
        errorText = "Unknown Error: $err";
        debugPrintStack(label: err.toString());
      }

      setState(() => isInProgress = false);
    }

    return CustomAlertDialog(
        title: Text(translate("Verification code")),
        contentBoxConstraints: BoxConstraints(maxWidth: 300),
        content: Column(
          children: [
            Offstage(
                offstage: user?.email == null,
                child: TextField(
                  decoration: InputDecoration(
                      labelText: "Email",
                      prefixIcon: Icon(Icons.email),
                      border: InputBorder.none),
                  readOnly: true,
                  controller: TextEditingController(text: user?.email),
                )),
            const SizedBox(height: 8),
            DialogTextField(
              title: '${translate("Verification code")}:',
              controller: code,
              errorText: errorText,
              focusNode: focusNode,
              helperText: translate('verification_tip'),
            ),
            CheckboxListTile(
              contentPadding: const EdgeInsets.all(0),
              dense: true,
              controlAffinity: ListTileControlAffinity.leading,
              title: Row(children: [
                Expanded(child: Text(translate("Trust this device")))
              ]),
              value: autoLogin,
              onChanged: (v) {
                if (v == null) return;
                setState(() => autoLogin = !autoLogin);
              },
            ),
            Offstage(
                offstage: !isInProgress,
                child: const LinearProgressIndicator()),
          ],
        ),
        actions: [
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: onVerify, child: Text(translate("Verify"))),
        ]);
  });

  return res;
}
