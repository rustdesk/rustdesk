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
  const LoginWidgetUserPass({
    Key? key,
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
                autoFocus: true,
                errorText: usernameMsg),
            DialogTextField(
                title: '${translate("Password")}:',
                obscureText: true,
                controller: pass,
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
  final bool autoFocus;
  final bool obscureText;
  final String? errorText;
  final TextEditingController controller;
  final FocusNode focusNode = FocusNode();

  DialogTextField(
      {Key? key,
      this.autoFocus = false,
      this.obscureText = false,
      this.errorText,
      required this.title,
      required this.controller})
      : super(key: key) {
    // todo mobile requestFocus, on mobile, widget will reload every time the text changes
    if (autoFocus && isDesktop) {
      Timer(Duration(milliseconds: 200), () => focusNode.requestFocus());
    }
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: TextField(
            decoration: InputDecoration(
                labelText: title,
                border: const OutlineInputBorder(),
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

  String? usernameMsg;
  String? passwordMsg;
  var isInProgress = false;
  final autoLogin = true.obs;
  final RxString curOP = ''.obs;

  return gFFI.dialogManager.show<bool>((setState, close) {
    cancel() {
      isInProgress = false;
      close(false);
    }

    onLogin() async {
      setState(() {
        usernameMsg = null;
        passwordMsg = null;
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
      if (username.text.isEmpty) {
        usernameMsg = translate('Username missed');
        cancel();
        return;
      }
      if (password.text.isEmpty) {
        passwordMsg = translate('Password missed');
        cancel();
        return;
      }
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
              bind.mainSetLocalOption(
                  key: 'access_token', value: resp.access_token!);
              close(true);
              return;
            }
            break;
          case HttpType.kAuthResTypeEmailCheck:
            break;
        }
      } on RequestException catch (err) {
        passwordMsg = translate(err.cause);
        debugPrintStack(label: err.toString());
        cancel();
        return;
      } catch (err) {
        passwordMsg = "Unknown Error";
        debugPrintStack(label: err.toString());
        cancel();
        return;
      }
      close();
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
      actions: [msgBoxButton(translate('Close'), cancel)],
      onCancel: cancel,
    );
  });
}
