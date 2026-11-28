import 'dart:convert';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class PrinterOptions {
  String action;
  List<String> printerNames;
  String printerName;
  String error;

  PrinterOptions(
      {required this.action,
      required this.printerNames,
      required this.printerName,
      this.error = ''});

  static PrinterOptions load({PdfPrinterList? pdfPrinters}) {
    var action = bind.mainGetLocalOption(key: kKeyPrinterIncomingJobAction);
    if (![
      kValuePrinterIncomingJobDismiss,
      kValuePrinterIncomingJobDefault,
      kValuePrinterIncomingJobSelected
    ].contains(action)) {
      action = kValuePrinterIncomingJobDefault;
    }

    final printerNames = pdfPrinters?.names ?? getPrinterNames();
    var selectedPrinterName = bind.mainGetLocalOption(key: kKeyPrinterSelected);
    if (pdfPrinters == null && !printerNames.contains(selectedPrinterName)) {
      if (action == kValuePrinterIncomingJobSelected) {
        action = kValuePrinterIncomingJobDefault;
        bind.mainSetLocalOption(
            key: kKeyPrinterIncomingJobAction,
            value: kValuePrinterIncomingJobDefault);
        if (printerNames.isEmpty) {
          selectedPrinterName = '';
        } else {
          selectedPrinterName = printerNames.first;
        }
        bind.mainSetLocalOption(
            key: kKeyPrinterSelected, value: selectedPrinterName);
      }
    }

    return PrinterOptions(
        action: action,
        printerNames: printerNames,
        printerName: selectedPrinterName,
        error: pdfPrinters?.error ?? '');
  }
}

class PdfPrinterList {
  final List<String> names;
  final String error;

  const PdfPrinterList({this.names = const [], this.error = ''});

  static Future<PdfPrinterList> load() async {
    try {
      final result =
          jsonDecode(await bind.mainGetCommon(key: 'pdf-printer-names'))
              as Map<String, dynamic>;
      return PdfPrinterList(
          names: List<String>.from(result['printers'] as List? ?? []),
          error: result['error'] as String? ?? '');
    } catch (error) {
      return PdfPrinterList(error: error.toString());
    }
  }
}
