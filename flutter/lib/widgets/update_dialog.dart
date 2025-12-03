import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:percent_indicator/linear_percent_indicator.dart';
import '../services/update_manager.dart';

class UpdateDialogController extends GetxController {
  final updateManager = UpdateManager();
  final isDownloading = false.obs;
  final downloadProgress = 0.obs;
  final updateInfo = Rxn<UpdateInfo>();

  @override
  void onInit() {
    super.onInit();
    setupCallbacks();
  }

  void setupCallbacks() {
    updateManager.onUpdateAvailable = (info) {
      updateInfo.value = info;
      showUpdateDialog(info);
    };

    updateManager.onDownloadProgress = (progress) {
      downloadProgress.value = progress;
    };

    updateManager.onUpdateInstalled = () {
      isDownloading.value = false;
      downloadProgress.value = 0;
    };

    updateManager.onError = (error) {
      isDownloading.value = false;
      Get.snackbar(
        'Ошибка',
        error,
        snackPosition: SnackPosition.BOTTOM,
        backgroundColor: Colors.red,
        colorText: Colors.white,
      );
    };
  }

  void checkForUpdates() {
    updateManager.checkForUpdates();
  }

  void downloadAndInstall(UpdateInfo info) {
    isDownloading.value = true;
    updateManager.downloadAndInstall(info);
  }

  void cancelDownload() {
    isDownloading.value = false;
    updateManager.cancelDownload();
  }

  void showUpdateDialog(UpdateInfo info) {
    Get.dialog(
      UpdateDialog(controller: this, updateInfo: info),
      barrierDismissible: false,
    );
  }
}

class UpdateDialog extends StatelessWidget {
  final UpdateDialogController controller;
  final UpdateInfo updateInfo;

  const UpdateDialog({
    required this.controller,
    required this.updateInfo,
  });

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Доступно обновление'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Версия: ${updateInfo.version}',
              style: const TextStyle(
                fontWeight: FontWeight.bold,
                fontSize: 16,
              ),
            ),
            const SizedBox(height: 12),
            const Text(
              'Изменения:',
              style: TextStyle(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 8),
            Text(
              updateInfo.releaseNotes.isEmpty
                  ? 'Новые улучшения и исправления'
                  : updateInfo.releaseNotes,
              maxLines: 5,
              overflow: TextOverflow.ellipsis,
            ),
            const SizedBox(height: 16),
            Obx(() {
              if (controller.isDownloading.value) {
                return Column(
                  children: [
                    LinearPercentIndicator(
                      lineHeight: 20.0,
                      percent: controller.downloadProgress.value / 100,
                      center: Text(
                        '${controller.downloadProgress.value}%',
                        style: const TextStyle(
                          color: Colors.white,
                          fontWeight: FontWeight.bold,
                          fontSize: 12,
                        ),
                      ),
                      linearStrokeCap: LinearStrokeCap.round,
                      progressColor: Colors.blue,
                    ),
                    const SizedBox(height: 12),
                    const Text(
                      'Загрузка обновления...',
                      style: TextStyle(color: Colors.grey),
                    ),
                  ],
                );
              }
              return const SizedBox.shrink();
            }),
          ],
        ),
      ),
      actions: [
        Obx(() {
          if (controller.isDownloading.value) {
            return TextButton(
              onPressed: () {
                controller.cancelDownload();
                Get.back();
              },
              child: const Text('Отмена'),
            );
          }
          return Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              TextButton(
                onPressed: () => Get.back(),
                child: const Text('Позже'),
              ),
              ElevatedButton(
                onPressed: () {
                  controller.downloadAndInstall(updateInfo);
                },
                child: const Text('Обновить'),
              ),
            ],
          );
        }),
      ],
    );
  }
}

/// Widget для кнопки проверки обновлений
class CheckUpdateButton extends StatelessWidget {
  final UpdateDialogController? controller;
  final EdgeInsets padding;
  final double iconSize;

  const CheckUpdateButton({
    this.controller,
    this.padding = const EdgeInsets.all(8.0),
    this.iconSize = 24.0,
  });

  @override
  Widget build(BuildContext context) {
    final updateController = controller ?? Get.put(UpdateDialogController());

    return Padding(
      padding: padding,
      child: Tooltip(
        message: 'Проверить обновления',
        child: IconButton(
          icon: const Icon(Icons.update),
          iconSize: iconSize,
          onPressed: () {
            updateController.checkForUpdates();
          },
        ),
      ),
    );
  }
}

/// Widget для отображения версии приложения с проверкой обновлений
class VersionInfoWidget extends StatelessWidget {
  final String currentVersion;
  final UpdateDialogController? controller;
  final bool showCheckButton;

  const VersionInfoWidget({
    required this.currentVersion,
    this.controller,
    this.showCheckButton = true,
  });

  @override
  Widget build(BuildContext context) {
    final updateController = controller ?? Get.put(UpdateDialogController());

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          'v$currentVersion',
          style: TextStyle(
            color: Colors.grey[600],
            fontSize: 12,
          ),
        ),
        if (showCheckButton) ...[
          const SizedBox(width: 8),
          CheckUpdateButton(
            controller: updateController,
            iconSize: 16,
            padding: const EdgeInsets.all(2),
          ),
        ],
      ],
    );
  }
}
