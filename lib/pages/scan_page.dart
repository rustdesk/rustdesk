import 'package:flutter/material.dart';
import 'package:qr_code_scanner/qr_code_scanner.dart';
import 'package:image_picker/image_picker.dart';
import 'package:image/image.dart' as img;
import 'package:zxing2/qrcode.dart';
import 'dart:io';
import 'dart:async';
import 'dart:convert';
import '../common.dart';
import '../models/model.dart';

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
      showToast('No permisssion');
    }
  }

  @override
  void dispose() {
    controller?.dispose();
    super.dispose();
  }

  void showServerSettingFromQr(String data) async {
    backToHome();
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
        showServerSettingsWithValue(host, '', key, api);
      });
    } catch (e) {
      showToast('Invalid QR code');
    }
  }
}

void showServerSettingsWithValue(
    String id, String relay, String key, String api) {
  final formKey = GlobalKey<FormState>();
  final id0 = FFI.getByName('option', 'custom-rendezvous-server');
  final relay0 = FFI.getByName('option', 'relay-server');
  final api0 = FFI.getByName('option', 'api-server');
  final key0 = FFI.getByName('option', 'key');
  DialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate('ID/Relay Server')),
      content: Form(
          key: formKey,
          child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                    TextFormField(
                      initialValue: id,
                      decoration: InputDecoration(
                        labelText: translate('ID Server'),
                      ),
                      validator: validate,
                      onSaved: (String? value) {
                        if (value != null) id = value.trim();
                      },
                    )
                  ] +
                  (isAndroid
                      ? [
                          TextFormField(
                            initialValue: relay,
                            decoration: InputDecoration(
                              labelText: translate('Relay Server'),
                            ),
                            validator: validate,
                            onSaved: (String? value) {
                              if (value != null) relay = value.trim();
                            },
                          )
                        ]
                      : []) +
                  [
                    TextFormField(
                      initialValue: api,
                      decoration: InputDecoration(
                        labelText: translate('API Server'),
                      ),
                      validator: validate,
                      onSaved: (String? value) {
                        if (value != null) api = value.trim();
                      },
                    ),
                    TextFormField(
                      initialValue: key,
                      decoration: InputDecoration(
                        labelText: 'Key',
                      ),
                      validator: null,
                      onSaved: (String? value) {
                        if (value != null) key = value.trim();
                      },
                    ),
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
          onPressed: () {
            if (formKey.currentState != null &&
                formKey.currentState!.validate()) {
              formKey.currentState!.save();
              if (id != id0)
                FFI.setByName('option',
                    '{"name": "custom-rendezvous-server", "value": "$id"}');
              if (relay != relay0)
                FFI.setByName(
                    'option', '{"name": "relay-server", "value": "$relay"}');
              if (key != key0)
                FFI.setByName('option', '{"name": "key", "value": "$key"}');
              if (api != api0)
                FFI.setByName(
                    'option', '{"name": "api-server", "value": "$api"}');
              FFI.ffiModel.updateUser();
              close();
            }
          },
          child: Text(translate('OK')),
        ),
      ],
    );
  });
}

String? validate(value) {
  value = value.trim();
  if (value.isEmpty) {
    return null;
  }
  final res = FFI.getByName('test_if_valid_server', value);
  return res.isEmpty ? null : res;
}
