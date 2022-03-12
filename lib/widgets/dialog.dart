import 'package:flutter/material.dart';
import '../common.dart';
import '../models/model.dart';

void clientClose() {
  msgBox('', 'Close', 'Are you sure to close the connection?');
}

void enterPasswordDialog(String id) {
  final controller = TextEditingController();
  var remember = FFI.getByName('remember', id) == 'true';
  DialogManager.show((context, setState) {
    return CustomAlertDialog(
      title: Text(translate('Password Required')),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        PasswordWidget(controller: controller),
        CheckboxListTile(
          contentPadding: const EdgeInsets.all(0),
          dense: true,
          controlAffinity: ListTileControlAffinity.leading,
          title: Text(
            translate('Remember password'),
          ),
          value: remember,
          onChanged: (v) {
            debugPrint("onChanged");
            if (v != null) {
              setState(() => remember = v);
            }
          },
        ),
      ]),
      actions: [
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            DialogManager.reset();
            Navigator.pop(globalKey.currentContext!);
          },
          child: Text(translate('Cancel')),
        ),
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            var text = controller.text.trim();
            if (text == '') return;
            FFI.login(text, remember);
            DialogManager.reset();
            showLoading(translate('Logging in...'));
          },
          child: Text(translate('OK')),
        ),
      ],
    );
  });
}

void wrongPasswordDialog(String id) {
  DialogManager.show((context, setState) => CustomAlertDialog(
          title: Text(translate('Wrong Password')),
          content: Text(translate('Do you want to enter again?')),
          actions: [
            TextButton(
              style: flatButtonStyle,
              onPressed: () {
                DialogManager.reset();
                Navigator.pop(globalKey.currentContext!);
              },
              child: Text(translate('Cancel')),
            ),
            TextButton(
              style: flatButtonStyle,
              onPressed: () {
                enterPasswordDialog(id);
              },
              child: Text(translate('Retry')),
            ),
          ]));
}

class PasswordWidget extends StatefulWidget {
  PasswordWidget({Key? key, required this.controller}) : super(key: key);

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
      obscureText: !_passwordVisible,
      //This will obscure text dynamically
      keyboardType: TextInputType.visiblePassword,
      decoration: InputDecoration(
        labelText: Translator.call('Password'),
        hintText: Translator.call('Enter your password'),
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
