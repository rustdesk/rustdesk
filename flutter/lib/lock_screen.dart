import 'package:flutter/material.dart';

class LockScreen extends StatefulWidget {
  final VoidCallback onUnlockSuccess;

  const LockScreen({super.key, required this.onUnlockSuccess});

  @override
  State<LockScreen> createState() => _LockScreenState();
}

class _LockScreenState extends State<LockScreen> {
  final TextEditingController _controller = TextEditingController();
  final String _correctPassword = '123456'; // 这里保存密码

  String? _errorText;

  void _tryUnlock() {
    if (_controller.text == _correctPassword) {
      setState(() {
        _errorText = null;
      });
      widget.onUnlockSuccess();
      _controller.clear();
    } else {
      setState(() {
        _errorText = '密码错误，请重试';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black87,
      body: Center(
        child: SizedBox(
          width: 280,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text(
                '请输入密码解锁',
                style: TextStyle(color: Colors.white, fontSize: 24),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _controller,
                obscureText: true,
                style: const TextStyle(color: Colors.white),
                decoration: InputDecoration(
                  filled: true,
                  fillColor: Colors.white10,
                  hintText: '密码',
                  hintStyle: const TextStyle(color: Colors.white54),
                  border: const OutlineInputBorder(),
                  errorText: _errorText,
                ),
                onSubmitted: (_) => _tryUnlock(),
              ),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: _tryUnlock,
                child: const Text('解锁'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
