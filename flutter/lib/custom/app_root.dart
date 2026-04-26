import 'package:flutter/material.dart';

import 'input/input_bridge.dart';

class AppRoot extends StatelessWidget {
  const AppRoot({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Tabby',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF2563EB),
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
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
      body: const Center(
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: 24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.pets, size: 80),
              SizedBox(height: 16),
              Text(
                'Tabby — Phase 1 scaffold mounted',
                textAlign: TextAlign.center,
                style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
              ),
              SizedBox(height: 12),
              Text(
                'Build with --dart-define=CUSTOM_UI=false to fall back to the vanilla RustDesk UI.',
                textAlign: TextAlign.center,
                style: TextStyle(color: Colors.grey),
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
