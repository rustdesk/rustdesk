import 'package:flutter/material.dart';
import 'dart:async';
import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:tuple/tuple.dart';
import 'dart:io';

final bool isZh = Platform.localeName == "zh_CN";

final langs = <String, Map<String, String>>{
  'zh': <String, String>{
    'Remote ID': '远程',
    'ID/Relay Server': 'ID/中继服务器',
    'About': '关于',
    'Mute': '静音',
    'ID Server': 'ID服务器',
    'Relay Server': '中继服务器',
    'Invalid IP': '无效IP',
    'Invalid format': '无效格式',
    'Cancel': '取消',
    'Close': '关闭',
    'Retry': '再试',
    'OK': '确认',
    'Password Required': '需要密码',
    'Please enter your password': '请输入密码',
    'Remember password': '记住密码',
    'Wrong Password': '密码错误',
    'Do you want to enter again?': '还想输入一次吗?',
    'Connection Error': '连接错误',
    'Error': '错误',
    'Reset by the peer': '连接被对方关闭',
    'Connecting...': '正在连接...',
    'Connection in progress. Please wait.': '连接进行中，请稍等。',
    'Please try 1 minute later': '一分钟后再试',
    'Login Error': '登录错误',
    'Successful': '成功',
    'Connected, waiting for image...': '已连接，等待画面传输...',
    'Custom Image Quality': '设置画面质量',
    'Privacy mode': '隐私模式',
    'Remove': '删除',
    'Adjust Window': '调节窗口',
    'Good image quality': '好画质',
    'Balanced': '一般画质',
    'Optimize reaction time': '优化反应时间',
    'Custom': '自定义画质',
    'Show remote cursor': '显示远程光标',
    'Disable clipboard': '禁止剪贴板',
    'Lock after session end': '断开后锁定远程电脑',
    'Insert': '插入',
    'Insert Lock': '锁定远程电脑',
    'Refresh': '刷新画面',
    'ID not exist': 'ID不存在',
    'Failed to connect to rendezvous server': '连接服务器失败',
    'Remote desktop is offline': '远程电脑不在线',
    'Key mismatch': 'Key不匹配',
    'Timeout': '连接超时',
    'Failed to connect to relay server': '无法连接到中继服务器',
    'Failed to connect via rendezvous server': '无法通过服务器建立连接',
    'Failed to make direct connection to remote desktop': '无法建立直接连接',
    'OS Password': '操作系统密码',
    'Paste': '粘贴',
    'Logging in...': '正在登录...',
    'Are you sure to close the connection?': '是否确认关闭连接？',
  },
  'en': <String, String>{}
};

String translate(name) {
  final tmp = isZh ? langs['zh'] : langs['en'];
  final v = tmp[name];
  return v != null ? v : name;
}

class MyTheme {
  MyTheme._();
  static const Color grayBg = Color(0xFFEEEEEE);
  static const Color white = Color(0xFFFFFFFF);
  static const Color accent = Color(0xFF0071FF);
  static const Color accent50 = Color(0x770071FF);
  static const Color accent80 = Color(0xAA0071FF);
  static const Color canvasColor = Color(0xFF212121);
  static const Color border = Color(0xFFCCCCCC);
}

void showLoading(String text, BuildContext context) {
  if (_hasDialog && context != null) {
    Navigator.pop(context);
  }
  dismissLoading();
  EasyLoading.show(status: text, maskType: EasyLoadingMaskType.black);
}

void dismissLoading() {
  EasyLoading.dismiss();
}

void showSuccess(String text) {
  dismissLoading();
  EasyLoading.showSuccess(text, maskType: EasyLoadingMaskType.black);
}

bool _hasDialog = false;
typedef BuildAlertDailog = Tuple3<Widget, Widget, List<Widget>> Function(
    void Function(void Function()));

Future<T> showAlertDialog<T>(BuildContext context, BuildAlertDailog build,
    [WillPopCallback onWillPop,
    bool barrierDismissible = false,
    double contentPadding = 20]) async {
  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  _hasDialog = true;
  var dialog = StatefulBuilder(builder: (context, setState) {
    var widgets = build(setState);
    if (onWillPop == null) onWillPop = () async => false;
    return WillPopScope(
        onWillPop: onWillPop,
        child: AlertDialog(
          title: widgets.item1,
          contentPadding: EdgeInsets.all(contentPadding),
          content: widgets.item2,
          actions: widgets.item3,
        ));
  });
  var res = await showDialog<T>(
      context: context,
      barrierDismissible: barrierDismissible,
      builder: (context) => dialog);
  _hasDialog = false;
  return res;
}

void msgbox(String type, String title, String text, BuildContext context,
    [bool hasCancel]) {
  var wrap = (String text, void Function() onPressed) => ButtonTheme(
      padding: EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      materialTapTargetSize: MaterialTapTargetSize
          .shrinkWrap, //limits the touch area to the button area
      minWidth: 0, //wraps child's width
      height: 0,
      child: FlatButton(
          focusColor: MyTheme.accent,
          onPressed: onPressed,
          child: Text(text, style: TextStyle(color: MyTheme.accent))));

  dismissLoading();
  if (_hasDialog) {
    Navigator.pop(context);
  }
  final buttons = [
    Expanded(child: Container()),
    wrap(translate('OK'), () {
      dismissLoading();
      Navigator.pop(context);
    })
  ];
  if (hasCancel == null) {
    hasCancel = type != 'error';
  }
  if (hasCancel) {
    buttons.insert(
        1,
        wrap(translate('Cancel'), () {
          dismissLoading();
        }));
  }
  EasyLoading.showWidget(
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate(title), style: TextStyle(fontSize: 21)),
          SizedBox(height: 20),
          Text(translate(text), style: TextStyle(fontSize: 15)),
          SizedBox(height: 20),
          Row(
            children: buttons,
          )
        ],
      ),
      maskType: EasyLoadingMaskType.black);
}

class PasswordWidget extends StatefulWidget {
  PasswordWidget({Key key, this.controller}) : super(key: key);

  final TextEditingController controller;

  @override
  _PasswordWidgetState createState() => _PasswordWidgetState();
}

class _PasswordWidgetState extends State<PasswordWidget> {
  bool _passwordVisible = false;
  @override
  Widget build(BuildContext context) {
    return TextField(
      autofocus: true,
      controller: widget.controller,
      obscureText: !_passwordVisible, //This will obscure text dynamically
      keyboardType: TextInputType.visiblePassword,
      decoration: InputDecoration(
        labelText: 'Password',
        hintText: 'Enter your password',
        // Here is key idea
        suffixIcon: IconButton(
          icon: Icon(
            // Based on passwordVisible state choose the icon
            _passwordVisible ? Icons.visibility : Icons.visibility_off,
            color: Theme.of(context).primaryColorDark,
          ),
          onPressed: () {
            // Update the state i.e. toogle the state of passwordVisible variable
            setState(() {
              _passwordVisible = !_passwordVisible;
            });
          },
        ),
      ),
    );
  }
}

Color str2color(String str, [alpha = 0xFF]) {
  var hash = 160 << 16 + 114 << 8 + 91;
  for (var i = 0; i < str.length; i += 1) {
    hash = str.codeUnitAt(i) + ((hash << 5) - hash);
  }
  return Color((hash & 0xFFFFFF) | (alpha << 24));
}
