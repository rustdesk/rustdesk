// RustDesk Flutter 客户端配置模板
// 你可以将此文件命名为 config.dart 并放在 flutter/lib/common/
// 用于存储服务器地址、key、ID 等信息

class RustDeskConfig {
  // 服务器地址
  static const String serverHost = "your.server.com";
  // 服务器端口
  static const int serverPort = 21118;
  // 客户端ID
  static const String clientId = "your-client-id";
  // 客户端key
  static const String clientKey = "your-client-key";
  // 其它自定义配置
  static const String customOption = "value";
}

// 在其它 Dart 文件中：
// import 'package:flutter/lib/common/config.dart';
// 使用 RustDeskConfig.serverHost 等访问配置
