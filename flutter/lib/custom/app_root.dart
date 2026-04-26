import 'package:flutter/material.dart';

import 'input/input_bridge.dart';
import 'theme/app_theme.dart';
import 'theme/tokens.dart';

class AppRoot extends StatelessWidget {
  const AppRoot({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Tabby',
      theme: AppTheme.dark,
      home: const _ScaffoldHome(),
    );
  }
}

class _ScaffoldHome extends StatelessWidget {
  const _ScaffoldHome();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Tabby')),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: AppTokens.spaceXl),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.pets, size: 80),
              const SizedBox(height: AppTokens.spaceLg),
              Text(
                'Tabby — Phase 1 scaffold mounted',
                textAlign: TextAlign.center,
                style: AppTokens.fontTitle.copyWith(
                  color: AppTokens.colorTextHigh,
                ),
              ),
              const SizedBox(height: AppTokens.spaceMd),
              Text(
                'Build with --dart-define=CUSTOM_UI=false to fall back to the vanilla RustDesk UI.',
                textAlign: TextAlign.center,
                style: AppTokens.fontBody.copyWith(
                  color: AppTokens.colorTextMid,
                ),
              ),
            ],
          ),
        ),
      ),
      floatingActionButton: const _PocButton(),
    );
  }
}

class _PocButton extends StatelessWidget {
  const _PocButton();

  @override
  Widget build(BuildContext context) {
    return FloatingActionButton.extended(
      icon: const Icon(Icons.keyboard_return),
      label: const Text('Send Esc'),
      onPressed: () async {
        await InputBridge.poc().tapKey('escape');
        if (!context.mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
                'FFI invoked. No active session — wire one up in Phase 2.'),
            duration: Duration(seconds: 2),
          ),
        );
      },
    );
  }
}
