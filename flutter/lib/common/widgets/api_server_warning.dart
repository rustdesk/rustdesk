import 'package:flutter/material.dart';

bool shouldWarnApiServer(String? apiServer, {String? idServer}) {
  final api = apiServer?.trim() ?? '';
  final id = idServer?.trim() ?? '';
  if (api.isEmpty && (id.isEmpty || id == '::1')) {
    return false;
  }
  final uri = Uri.tryParse(api.isEmpty ? 'http://$id' : api);
  if (uri == null || uri.host.isEmpty) {
    return true;
  }
  if (uri.scheme == 'https') {
    return false;
  }
  return uri.scheme != 'http' ||
      !const {'localhost', '127.0.0.1', '::1'}.contains(uri.host);
}

class ApiServerWarning extends StatelessWidget {
  final TextEditingController controller;
  final TextEditingController idController;
  final String message;
  final String emptyMessage;

  const ApiServerWarning({
    super.key,
    required this.controller,
    required this.idController,
    required this.message,
    required this.emptyMessage,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Listenable.merge([controller, idController]),
      builder: (context, child) {
        if (!shouldWarnApiServer(controller.text,
            idServer: idController.text)) {
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
