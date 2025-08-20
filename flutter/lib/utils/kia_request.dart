import 'dart:convert';
import 'dart:io';

import 'package:flutter_hbb/common.dart';
import 'http_service.dart' as http;

class KiaRequest {
  final model = gFFI.serverModel;

  final api =
      "http://localhost:48080/app-api/rdm/rustdesk-client/upload-client-info";

  final headers = {
    'Content-Type': "application/json",
    'tenant-id': "1",
  };

  var id = translate("Generating ...");
  var passwd = translate("Generating ...");

  addListener() {
    listener() {
      if (model.serverId.text != id || model.serverPasswd.text != passwd) {
        uploadServerKey();
        id = model.serverId.text;
        passwd = model.serverPasswd.text;
      }
    }

    model.addListener(listener);
  }

  uploadServerKey() async {
    Map<String, dynamic> body = {
      'clientId': model.serverId.text,
      'clientPasswd': model.serverPasswd.text,
      'macAddress': "test",
      'machineCode': "test",
    };
    if (isLinux) {
      body['macAddress'] = await _getMacAddress();
      body['machineCode'] = await _getMachineCode();
    }
    await http.post(Uri.parse(api), headers: headers, body: jsonEncode(body));
  }

  // 获取指定网络接口的 MAC 地址（默认取 eth0 或 wlan0）
  static Future<String?> _getMacAddress({String interface = 'eth0'}) async {
    try {
      // 检查接口是否存在
      final interfacePath = '/sys/class/net/$interface';
      if (!await Directory(interfacePath).exists()) {
        // 若 eth0 不存在，尝试 wlan0（无线网卡）
        if (interface == 'eth0') {
          return _getMacAddress(interface: 'wlan0');
        }
        return null;
      }
      // 读取 MAC 地址文件
      final result = await Process.run(
        'cat',
        ['/sys/class/net/$interface/address'],
      );

      if (result.exitCode == 0) {
        return result.stdout.toString().trim(); // 去除换行符
      } else {
        print('获取 MAC 失败：${result.stderr}');
        return null;
      }
    } catch (e) {
      print('异常：$e');
      return null;
    }
  }

  static Future<String?> _getMachineCode() async {
    final filePath = '/bundle/config.json';
    final file = File(filePath);
    try {
      // 检查文件是否存在
      if (!await file.exists()) {
        print('错误：文件 $filePath 不存在');
        return null;
      }

      // 检查文件是否可读
      final stat = await file.stat();
      if ((stat.mode & 0x4) == 0) {
        print('错误：没有读取 $filePath 的权限');
        return null;
      }

      // 读取文件内容
      final jsonContent = await file.readAsString();
      // 解析 JSON 内容
      final jsonData = json.decode(jsonContent) as Map<String, dynamic>;
      return jsonData["machineCode"];
    } on FileSystemException catch (e) {
      print('文件操作异常：${e.message}');
      print('错误路径：${e.path}');
    } on FormatException catch (e) {
      print('JSON 格式错误：${e.message}');
    } catch (e) {
      print('未知错误：$e');
    }
    return null;
  }
}
