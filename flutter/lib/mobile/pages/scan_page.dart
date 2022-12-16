import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:image/image.dart' as img;
import 'package:image_picker/image_picker.dart';
import 'package:qr_code_scanner/qr_code_scanner.dart';
import 'package:zxing2/qrcode.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

class ScanPage extends StatefulWidget {
  @override
  _ScanPageState createState() => _ScanPageState();
}

class _ScanPageState extends State<ScanPage> {
  QRViewController? controller;
  final GlobalKey qrKey = GlobalKey(debugLabel: 'QR');

  // In order to get hot reload to work we need to pause the camera if the platform
  // is android, or resume the camera if the platform is iOS.
  @override
  void reassemble() {
    super.reassemble();
    if (isAndroid) {
      controller!.pauseCamera();
    }
    controller!.resumeCamera();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        appBar: AppBar(
          title: const Text('Scan QR'),
          actions: [
            IconButton(
                color: Colors.white,
                icon: Icon(Icons.image_search),
                iconSize: 32.0,
                onPressed: () async {
                  final ImagePicker _picker = ImagePicker();
                  final XFile? file =
                      await _picker.pickImage(source: ImageSource.gallery);
                  if (file != null) {
                    var image = img.decodeNamedImage(
                        File(file.path).readAsBytesSync(), file.path)!;

                    LuminanceSource source = RGBLuminanceSource(
                        image.width,
                        image.height,
                        image
                            .getBytes(format: img.Format.abgr)
                            .buffer
                            .asInt32List());
                    var bitmap = BinaryBitmap(HybridBinarizer(source));

                    var reader = QRCodeReader();
                    try {
                      var result = reader.decode(bitmap);
                      showServerSettingFromQr(result.text);
                    } catch (e) {
                      showToast('No QR code found');
                    }
                  }
                }),
            IconButton(
                color: Colors.yellow,
                icon: Icon(Icons.flash_on),
                iconSize: 32.0,
                onPressed: () async {
                  await controller?.toggleFlash();
                }),
            IconButton(
              color: Colors.white,
              icon: Icon(Icons.switch_camera),
              iconSize: 32.0,
              onPressed: () async {
                await controller?.flipCamera();
              },
            ),
          ],
        ),
        body: _buildQrView(context));
  }

  Widget _buildQrView(BuildContext context) {
    // For this example we check how width or tall the device is and change the scanArea and overlay accordingly.
    var scanArea = (MediaQuery.of(context).size.width < 400 ||
            MediaQuery.of(context).size.height < 400)
        ? 150.0
        : 300.0;
    // To ensure the Scanner view is properly sizes after rotation
    // we need to listen for Flutter SizeChanged notification and update controller
    return QRView(
      key: qrKey,
      onQRViewCreated: _onQRViewCreated,
      overlay: QrScannerOverlayShape(
          borderColor: Colors.red,
          borderRadius: 10,
          borderLength: 30,
          borderWidth: 10,
          cutOutSize: scanArea),
      onPermissionSet: (ctrl, p) => _onPermissionSet(context, ctrl, p),
    );
  }

  void _onQRViewCreated(QRViewController controller) {
    setState(() {
      this.controller = controller;
    });
    controller.scannedDataStream.listen((scanData) {
      if (scanData.code != null) {
        showServerSettingFromQr(scanData.code!);
      }
    });
  }

  void _onPermissionSet(BuildContext context, QRViewController ctrl, bool p) {
    if (!p) {
      showToast('No permission');
    }
  }

  @override
  void dispose() {
    controller?.dispose();
    super.dispose();
  }

  void showServerSettingFromQr(String data) async {
    closeConnection();
    await controller?.pauseCamera();
    if (!data.startsWith('config=')) {
      showToast('Invalid QR code');
      return;
    }
    try {
      Map<String, dynamic> values = json.decode(data.substring(7));
      var host = values['host'] != null ? values['host'] as String : '';
      var key = values['key'] != null ? values['key'] as String : '';
      var api = values['api'] != null ? values['api'] as String : '';
      Timer(Duration(milliseconds: 60), () {
        showServerSettingsWithValue(host, '', key, api, gFFI.dialogManager);
      });
    } catch (e) {
      showToast('Invalid QR code');
    }
  }
}

void showServerSettingsWithValue(String id, String relay, String key,
    String api, OverlayDialogManager dialogManager) async {
  Map<String, dynamic> oldOptions = jsonDecode(await bind.mainGetOptions());
  String id0 = oldOptions['custom-rendezvous-server'] ?? "";
  String relay0 = oldOptions['relay-server'] ?? "";
  String api0 = oldOptions['api-server'] ?? "";
  String key0 = oldOptions['key'] ?? "";
  var isInProgress = false;
  final idController = TextEditingController(text: id);
  final relayController = TextEditingController(text: relay);
  final apiController = TextEditingController(text: api);

  String? idServerMsg;
  String? relayServerMsg;
  String? apiServerMsg;

  dialogManager.show((setState, close) {
    Future<bool> validate() async {
      if (idController.text != id) {
        final res = await validateAsync(idController.text);
        setState(() => idServerMsg = res);
        if (idServerMsg != null) return false;
        id = idController.text;
      }
      if (relayController.text != relay) {
        relayServerMsg = await validateAsync(relayController.text);
        if (relayServerMsg != null) return false;
        relay = relayController.text;
      }
      if (apiController.text != relay) {
        apiServerMsg = await validateAsync(apiController.text);
        if (apiServerMsg != null) return false;
        api = apiController.text;
      }
      return true;
    }

    return CustomAlertDialog(
      title: Text(translate('ID/Relay Server')),
      content: Form(
          child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                    TextFormField(
                      controller: idController,
                      decoration: InputDecoration(
                          labelText: translate('ID Server'),
                          errorText: idServerMsg),
                    )
                  ] +
                  (isAndroid
                      ? [
                          TextFormField(
                            controller: relayController,
                            decoration: InputDecoration(
                                labelText: translate('Relay Server'),
                                errorText: relayServerMsg),
                          )
                        ]
                      : []) +
                  [
                    TextFormField(
                      controller: apiController,
                      decoration: InputDecoration(
                        labelText: translate('API Server'),
                      ),
                      autovalidateMode: AutovalidateMode.onUserInteraction,
                      validator: (v) {
                        if (v != null && v.length > 0) {
                          if (!(v.startsWith('http://') ||
                              v.startsWith("https://"))) {
                            return translate("invalid_http");
                          }
                        }
                        return apiServerMsg;
                      },
                    ),
                    TextFormField(
                      initialValue: key,
                      decoration: InputDecoration(
                        labelText: 'Key',
                      ),
                      onChanged: (String? value) {
                        if (value != null) key = value.trim();
                      },
                    ),
                    Offstage(
                        offstage: !isInProgress,
                        child: LinearProgressIndicator())
                  ])),
      actions: [
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            close();
          },
          child: Text(translate('Cancel')),
        ),
        TextButton(
          style: flatButtonStyle,
          onPressed: () async {
            setState(() {
              idServerMsg = null;
              relayServerMsg = null;
              apiServerMsg = null;
              isInProgress = true;
            });
            if (await validate()) {
              if (id != id0) {
                if (id0.isNotEmpty) {
                  await gFFI.userModel.logOut();
                }
                bind.mainSetOption(key: "custom-rendezvous-server", value: id);
              }
              if (relay != relay0) {
                bind.mainSetOption(key: "relay-server", value: relay);
              }
              if (key != key0) bind.mainSetOption(key: "key", value: key);
              if (api != api0) {
                bind.mainSetOption(key: "api-server", value: api);
              }
              close();
            }
            setState(() {
              isInProgress = false;
            });
          },
          child: Text(translate('OK')),
        ),
      ],
    );
  });
}

Future<String?> validateAsync(String value) async {
  value = value.trim();
  if (value.isEmpty) {
    return null;
  }
  final res = await bind.mainTestIfValidServer(server: value);
  return res.isEmpty ? null : res;
}
