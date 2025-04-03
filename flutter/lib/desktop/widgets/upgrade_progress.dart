import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class UpgradeProgress extends StatefulWidget {
  final String releasePageUrl;
  final String downloadUrl;
  final SimpleWrapper downloadId;
  UpgradeProgress(this.releasePageUrl, this.downloadUrl, this.downloadId,
      {Key? key})
      : super(key: key);

  @override
  State<UpgradeProgress> createState() => UpgradeProgressState();
}

class UpgradeProgressState extends State<UpgradeProgress> {
  Timer? _timer;
  String get downloadUrl => widget.downloadUrl;
  int? totalSize;
  int downloadedSize = 0;
  String error = '';
  int getDataFailedCount = 0;
  final String _eventKeyDownloadNewVersion = 'download-new-version';

  @override
  void initState() {
    super.initState();
    platformFFI.registerEventHandler(_eventKeyDownloadNewVersion,
        _eventKeyDownloadNewVersion, handleDownloadNewVersion,
        replace: true);
    bind.mainSetCommon(key: 'download-new-version', value: downloadUrl);
  }

  @override
  void dispose() {
    _timer?.cancel();
    platformFFI.unregisterEventHandler(
        _eventKeyDownloadNewVersion, _eventKeyDownloadNewVersion);
    super.dispose();
  }

  Future<void> handleDownloadNewVersion(Map<String, dynamic> evt) async {
    if (evt.containsKey('id')) {
      widget.downloadId.value = evt['id'] as String;
      _timer = Timer.periodic(const Duration(milliseconds: 300), (timer) {
        _updateDownloadData();
      });
    } else {
      if (evt.containsKey('error')) {
        _onError(evt['error'] as String);
      } else {
        // unreachable
        _onError('$evt');
      }
    }
  }

  void _onError(String error) {
    msgBox(
        gFFI.sessionId,
        'custom-nocancel',
        'Error',
        'download-new-veresion-failed-tip',
        widget.releasePageUrl,
        gFFI.dialogManager);
  }

  void _updateDownloadData() {
    String downloadData = bind.mainGetCommonSync(key: 'download-data-${widget.downloadId.value}');
    if (downloadData.startsWith('error:')) {
      error = downloadData.substring('error:'.length);
    } else {
      try {
        jsonDecode(downloadData).forEach((key, value) {
          if (key == 'total_size') {
            if (value != null && value is int) {
              totalSize = value;
            }
          } else if (key == 'downloaded_size') {
            downloadedSize = value as int;
          } else if (key == 'error') {
            if (value != null) {
              error = value.toString();
            }
          }
        });
      } catch (e) {
        getDataFailedCount += 1;
        debugPrint('Failed to get download data $downloadUrl, error $e');
        if (getDataFailedCount > 3) {
          error = e.toString();
        }
      }
    }
    if (error != '') {
      _onError(error);
    } else {
      if (totalSize != null && downloadedSize >= totalSize!) {
        _timer?.cancel();
        _timer = null;
        bind.mainSetCommon(key: 'remove-downloader', value: widget.downloadId.value);
        if (totalSize == 0) {
          _onError('The download file size is 0.');
        } else {
          setState(() {});
          Future.delayed(const Duration(milliseconds: 500), () {
            msgBox(gFFI.sessionId, 'custom-nocancel', '{$appName} Upgrade',
                '{$appName}-to-upgrade-tip', '', gFFI.dialogManager);
            Future.delayed(const Duration(milliseconds: 1000), () {
              bind.mainSetCommon(key: 'upgrade-me', value: '');
            });
          });
        }
      } else {
        setState(() {});
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return onDownloading(context);
  }

  Widget onDownloading(BuildContext context) {
    final value = totalSize == null
        ? 0.0
        : (totalSize == 0 ? 1.0 : downloadedSize / totalSize!);
    return LinearProgressIndicator(
      value: value,
      minHeight: 20,
      borderRadius: BorderRadius.circular(5),
      backgroundColor: Colors.grey[300],
      valueColor: const AlwaysStoppedAnimation<Color>(Colors.blue),
    );
  }
}
