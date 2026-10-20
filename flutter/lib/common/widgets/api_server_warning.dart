import 'package:flutter/material.dart';

bool shouldWarnApiServer(String? apiServer) {
  final uri = Uri.tryParse(apiServer?.trim() ?? '');
  if (uri == null || uri.host.isEmpty || uri.scheme != 'http') {
    return false;
  }
  return !const {'localhost', '127.0.0.1', '::1'}.contains(uri.host);
}

class ApiServerWarning extends StatelessWidget {
  final TextEditingController controller;
  final TextEditingController idController;
  final String message;
  final String emptyMessage;
  final String Function({required String api, required String custom})
      resolveApiServer;

  const ApiServerWarning({
    super.key,
    required this.controller,
    required this.idController,
    required this.message,
    required this.emptyMessage,
    required this.resolveApiServer,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Listenable.merge([controller, idController]),
      builder: (context, child) {
        final apiServer = controller.text;
        final idServer = idController.text;
        final effectiveApiServer =
            resolveApiServer(api: apiServer.trim(), custom: idServer.trim());
        if (!shouldWarnApiServer(effectiveApiServer)) {
          return const SizedBox.shrink();
        }
        return Align(
          alignment: AlignmentDirectional.centerStart,
          child: Text(
            controller.text.trim().isEmpty ? emptyMessage : message,
            style: TextStyle(color: Colors.orange.shade700),
          ),
        );
      },
    );
  }
}
