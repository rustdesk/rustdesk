import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';

String _resolveApiServer({required String api, required String custom}) {
  final key = 'resolve-api-server:${jsonEncode([api, custom])}';
  final response = jsonDecode(bind.mainGetCommonSync(key: key));
  if (response['error'] != null) {
    throw FormatException('Failed to resolve API server: ${response['error']}');
  }
  return response['url'] as String;
}

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
    this.resolveApiServer = _resolveApiServer,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Listenable.merge([controller, idController]),
      builder: (context, child) {
        final apiServer = controller.text;
        final idServer = idController.text;
        final String effectiveApiServer;
        try {
          effectiveApiServer =
              resolveApiServer(api: apiServer.trim(), custom: idServer.trim());
        } catch (error, stack) {
          FlutterError.reportError(FlutterErrorDetails(
            exception: error,
            stack: stack,
            context: ErrorDescription('while resolving the API server warning'),
          ));
          return const SizedBox.shrink();
        }
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
