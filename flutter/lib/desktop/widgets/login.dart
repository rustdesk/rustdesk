import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../../common.dart';
import '../widgets/button.dart';

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
          padding: const EdgeInsets.fromLTRB(10, 0, 10, 0),
          child: Obx(() => ElevatedButton(
                style: ElevatedButton.styleFrom(
                  primary: curOP.value.isEmpty || curOP.value == op
                      ? primaryColor
                      : Colors.grey,
                ).copyWith(elevation: ButtonStyleButton.allOrNull(0.0)),
                onPressed:
                    curOP.value.isEmpty || curOP.value == op ? onTap : null,
                child: Stack(children: [
                  // to-do: translate
                  Center(child: Text('Continue with $op')),
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
  const WidgetOP({
    Key? key,
    required this.config,
    required this.curOP,
  }) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _WidgetOPState();
  }
}

class _WidgetOPState extends State<WidgetOP> {
  Timer? _updateTimer;
  String _stateMsg = '';
  String _stateFailedMsg = '';
  String _url = '';
  String _username = '';

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
      final String failedMsg = resultMap['failed_msg'];
      // to-do: test null url
      final String url = resultMap['url'];
      if (_stateMsg != stateMsg) {
        if (_url.isEmpty && url.isNotEmpty) {
          launchUrl(Uri.parse(url));
          _url = url;
        }
        setState(() {
          _stateMsg = stateMsg;
          _stateFailedMsg = failedMsg;
        });
      }
    });
  }

  _resetState() {
    _stateMsg = '';
    _stateFailedMsg = '';
    _url = '';
    _username = '';
  }

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
        constraints: const BoxConstraints(minWidth: 500),
        child: Column(
          children: [
            ButtonOP(
              op: widget.config.op,
              curOP: widget.curOP,
              iconWidth: widget.config.iconWidth,
              primaryColor: str2color(widget.config.op, 0x7f),
              height: 40,
              onTap: () {
                widget.curOP.value = widget.config.op;
                bind.mainAccountAuth(op: widget.config.op);
                _beginQueryState();
              },
            ),
            Obx(() => Offstage(
                offstage: widget.curOP.value != widget.config.op,
                child: Text(
                  _stateMsg,
                  style: TextStyle(fontSize: 12),
                ))),
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
        ));
  }
}

class LoginWidgetOP extends StatelessWidget {
  final List<ConfigOP> ops;
  final RxString curOP = ''.obs;

  LoginWidgetOP({
    Key? key,
    required this.ops,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var children = ops
        .map((op) => [
              WidgetOP(
                config: op,
                curOP: curOP,
              ),
              const Divider()
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
  final Function(String, String) onLogin;
  const LoginWidgetUserPass({
    Key? key,
    required this.username,
    required this.pass,
    required this.usernameMsg,
    required this.passMsg,
    required this.isInProgress,
    required this.onLogin,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var userController = TextEditingController(text: username);
    var pwdController = TextEditingController(text: pass);
    return ConstrainedBox(
      constraints: const BoxConstraints(minWidth: 500),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(
            height: 8.0,
          ),
          Row(
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
          const SizedBox(
            height: 8.0,
          ),
          Row(
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
                height: 50,
                padding: const EdgeInsets.fromLTRB(10, 0, 10, 0),
                child: ElevatedButton(
                  child: const Text(
                    'Login',
                    style: TextStyle(fontSize: 18),
                  ),
                  onPressed: () {
                    onLogin(userController.text, pwdController.text);
                  },
                ),
              ),
            ),
          ]),
        ],
      ),
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
        if (isInProgress) {
          setState(() {
            isInProgress = false;
          });
        }
      }

      username = username0;
      pass = pass0;
      if (username.isEmpty) {
        usernameMsg = translate('Username missed');
        debugPrint('REMOVE ME ====================== username empty');
        cancel();
        return;
      }
      if (pass.isEmpty) {
        passMsg = translate('Password missed');
        debugPrint('REMOVE ME ====================== password empty');
        cancel();
        return;
      }
      try {
        final resp = await gFFI.userModel.login(username, pass);
        if (resp.containsKey('error')) {
          passMsg = resp['error'];
          debugPrint('REMOVE ME ====================== password error');
          cancel();
          return;
        }
        // {access_token: eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJndWlkIjoiMDFkZjQ2ZjgtZjg3OS00MDE0LTk5Y2QtMGMwYzM2MmViZGJlIiwiZXhwIjoxNjYxNDg2NzYwfQ.GZpe1oI8TfM5yTYNrpcwbI599P4Z_-b2GmnwNl2Lr-w,
        // token_type: Bearer, user: {id: , name: admin, email: null, note: null, status: null, grp: null, is_admin: true}}
        debugPrint('$resp');
        completer.complete(true);
      } catch (err) {
        debugPrint(err.toString());
        debugPrint(
            'REMOVE ME ====================== login error ${err.toString()}');
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
              onLogin: onLogin,
            ),
            const SizedBox(
              height: 8.0,
            ),
            const Center(
                child: Text(
              'or',
              style: TextStyle(fontSize: 16),
            )),
            const SizedBox(
              height: 8.0,
            ),
            LoginWidgetOP(ops: [
              ConfigOP(op: 'Github', iconWidth: 24),
              ConfigOP(op: 'Google', iconWidth: 24),
              ConfigOP(op: 'Okta', iconWidth: 46),
            ]),
          ],
        ),
      ),
      actions: [
        TextButton(onPressed: cancel, child: Text(translate('Cancel'))),
      ],
      onCancel: cancel,
    );
  });
  return completer.future;
}
