import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';

final kMidButtonPadding = const EdgeInsets.fromLTRB(15, 0, 15, 0);

class _IconOP extends StatelessWidget {
  final String icon;
  final double iconWidth;
  const _IconOP({Key? key, required this.icon, required this.iconWidth})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 4.0),
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
      Expanded(
        child: Container(
          height: height,
          padding: kMidButtonPadding,
          child: Obx(() => ElevatedButton(
                style: ElevatedButton.styleFrom(
                  primary: curOP.value.isEmpty || curOP.value == op
                      ? primaryColor
                      : Colors.grey,
                ).copyWith(elevation: ButtonStyleButton.allOrNull(0.0)),
                onPressed:
                    curOP.value.isEmpty || curOP.value == op ? onTap : null,
                child: Stack(children: [
                  Center(child: Text('${translate("Continue with")} $op')),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: SizedBox(
                        width: 120,
                        child: _IconOP(
                          icon: op,
                          iconWidth: iconWidth,
                        )),
                  ),
                ]),
              )),
        ),
      )
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
  String _FailedMsg = '';
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
      if (_stateMsg != stateMsg || _FailedMsg != failedMsg) {
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
          _FailedMsg = failedMsg;
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
    _FailedMsg = '';
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
            _FailedMsg = '';
          }
          return Offstage(
              offstage:
                  _FailedMsg.isEmpty && widget.curOP.value != widget.config.op,
              child: Row(
                children: [
                  Text(
                    _stateMsg,
                    style: TextStyle(fontSize: 12),
                  ),
                  SizedBox(width: 8),
                  Text(
                    _FailedMsg,
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
        child: Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.spaceAround,
      children: children,
    ));
  }
}

class LoginWidgetUserPass extends StatelessWidget {
  final String username;
  final String pass;
  final String usernameMsg;
  final String passMsg;
  final bool isInProgress;
  final RxString curOP;
  final Function(String, String) onLogin;
  const LoginWidgetUserPass({
    Key? key,
    required this.username,
    required this.pass,
    required this.usernameMsg,
    required this.passMsg,
    required this.isInProgress,
    required this.curOP,
    required this.onLogin,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var userController = TextEditingController(text: username);
    var pwdController = TextEditingController(text: pass);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SizedBox(
          height: 8.0,
        ),
        Container(
          padding: kMidButtonPadding,
          child: Row(
            children: [
              ConstrainedBox(
                  constraints: const BoxConstraints(minWidth: 100),
                  child: Text(
                    '${translate("Username")}:',
                    textAlign: TextAlign.start,
                  ).marginOnly(bottom: 16.0)),
              const SizedBox(
                width: 24.0,
              ),
              Expanded(
                child: TextField(
                  decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      errorText: usernameMsg.isNotEmpty ? usernameMsg : null),
                  controller: userController,
                  focusNode: FocusNode()..requestFocus(),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(
          height: 8.0,
        ),
        Container(
          padding: kMidButtonPadding,
          child: Row(
            children: [
              ConstrainedBox(
                  constraints: const BoxConstraints(minWidth: 100),
                  child: Text('${translate("Password")}:')
                      .marginOnly(bottom: 16.0)),
              const SizedBox(
                width: 24.0,
              ),
              Expanded(
                child: TextField(
                  obscureText: true,
                  decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      errorText: passMsg.isNotEmpty ? passMsg : null),
                  controller: pwdController,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(
          height: 4.0,
        ),
        Offstage(
            offstage: !isInProgress, child: const LinearProgressIndicator()),
        const SizedBox(
          height: 12.0,
        ),
        Row(children: [
          Expanded(
            child: Container(
              height: 38,
              padding: kMidButtonPadding,
              child: Obx(() => ElevatedButton(
                    style: curOP.value.isEmpty || curOP.value == 'rustdesk'
                        ? null
                        : ElevatedButton.styleFrom(
                            primary: Colors.grey,
                          ),
                    child: Text(
                      translate('Login'),
                      style: TextStyle(fontSize: 16),
                    ),
                    onPressed: curOP.value.isEmpty || curOP.value == 'rustdesk'
                        ? () {
                            onLogin(userController.text, pwdController.text);
                          }
                        : null,
                  )),
            ),
          ),
        ]),
      ],
    );
  }
}

/// common login dialog for desktop
/// call this directly
Future<bool> loginDialog() async {
  String username = '';
  var usernameMsg = '';
  String pass = '';
  var passMsg = '';
  var isInProgress = false;
  var completer = Completer<bool>();
  final RxString curOP = ''.obs;

  gFFI.dialogManager.show((setState, close) {
    cancel() {
      isInProgress = false;
      completer.complete(false);
      close();
    }

    onLogin(String username0, String pass0) async {
      setState(() {
        usernameMsg = '';
        passMsg = '';
        isInProgress = true;
      });
      cancel() {
        curOP.value = '';
        if (isInProgress) {
          setState(() {
            isInProgress = false;
          });
        }
      }

      curOP.value = 'rustdesk';
      username = username0;
      pass = pass0;
      if (username.isEmpty) {
        usernameMsg = translate('Username missed');
        cancel();
        return;
      }
      if (pass.isEmpty) {
        passMsg = translate('Password missed');
        cancel();
        return;
      }
      try {
        final resp = await gFFI.userModel.login(username, pass);
        if (resp.containsKey('error')) {
          passMsg = resp['error'];
          cancel();
          return;
        }
        // {access_token: eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJndWlkIjoiMDFkZjQ2ZjgtZjg3OS00MDE0LTk5Y2QtMGMwYzM2MmViZGJlIiwiZXhwIjoxNjYxNDg2NzYwfQ.GZpe1oI8TfM5yTYNrpcwbI599P4Z_-b2GmnwNl2Lr-w,
        // token_type: Bearer, user: {id: , name: admin, email: null, note: null, status: null, grp: null, is_admin: true}}
        debugPrint('$resp');
        completer.complete(true);
      } catch (err) {
        debugPrint(err.toString());
        cancel();
        return;
      }
      close();
    }

    return CustomAlertDialog(
      title: Text(translate('Login')),
      content: ConstrainedBox(
        constraints: const BoxConstraints(minWidth: 500),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const SizedBox(
              height: 8.0,
            ),
            LoginWidgetUserPass(
              username: username,
              pass: pass,
              usernameMsg: usernameMsg,
              passMsg: passMsg,
              isInProgress: isInProgress,
              curOP: curOP,
              onLogin: onLogin,
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
                completer.complete(true);
                close();
              },
            ),
          ],
        ),
      ),
      actions: [msgBoxButton(translate('Close'), cancel)],
      onCancel: cancel,
    );
  });
  return completer.future;
}
