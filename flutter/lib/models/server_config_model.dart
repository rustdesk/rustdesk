import 'dart:convert';

/// 多服务器配置模型，仅在 Flutter 侧管理，不触及 FFI。
class ServerConfig {
  final String id; // UUID
  final String name; // 配置名称/标签
  final String idServer;
  final String relayServer;
  final String apiServer;
  final String key;

  const ServerConfig({
    required this.id,
    required this.name,
    required this.idServer,
    required this.relayServer,
    required this.apiServer,
    required this.key,
  });

  factory ServerConfig.fromJson(Map<String, dynamic> json) {
    return ServerConfig(
      id: json['id'] ?? '',
      name: json['name'] ?? '',
      idServer: json['idServer'] ?? '',
      relayServer: json['relayServer'] ?? '',
      apiServer: json['apiServer'] ?? '',
      key: json['key'] ?? '',
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'id': id,
      'name': name,
      'idServer': idServer,
      'relayServer': relayServer,
      'apiServer': apiServer,
      'key': key,
    };
  }

  /// 便于安全写入本地存储的 JSON 字符串。
  String toJsonString() => jsonEncode(toJson());
}
