import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:image/image.dart' as img;
import 'package:image_picker/image_picker.dart';
import 'package:qr_code_scanner/qr_code_scanner.dart';
import 'package:zxing2/qrcode.dart';

import '../../common.dart';
import '../widgets/dialog.dart';

class ScanPage extends StatefulWidget {
  @override
  State<ScanPage> createState() => _ScanPageState();
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
                  final ImagePicker picker = ImagePicker();
                  final XFile? file =
                      await picker.pickImage(source: ImageSource.gallery);
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
      final sc = ServerConfig.decode(data.substring(7));
      Timer(Duration(milliseconds: 60), () {
        showServerSettingsWithValue(sc.idServer, sc.relayServer, sc.key,
            sc.apiServer, gFFI.dialogManager);
      });
    } catch (e) {
      showToast('Invalid QR code');
    }
  }
}
