import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import '../../common.dart';

typedef KBChosenCallback = Future<bool> Function(String);

const double _kImageMarginVertical = 6.0;
const double _kImageMarginHorizontal = 10.0;
const double _kImageBoarderWidth = 4.0;
const double _kImagePaddingWidth = 4.0;
const Color _kImageBorderColor = Color.fromARGB(125, 202, 247, 2);
const double _kBorderRadius = 6.0;
const String _kKBLayoutTypeUnknown = 'Unknown';
const String _kKBLayoutTypeANSI = 'ANSI';
const String _kKBLayoutTypeISO = 'ISO';
const String _kKBLayoutTypeJIS = 'JIS';
const String _kKBLayoutTypeNotISO = 'Not ISO';

const _kKBLayoutImageMap = {
  _kKBLayoutTypeUnknown: 'kb_layout_not_iso',
  _kKBLayoutTypeANSI: 'kb_layout_not_iso',
  _kKBLayoutTypeISO: 'kb_layout_iso',
  _kKBLayoutTypeJIS: 'kb_layout_not_iso',
};

const _kKBLayoutTypes = [
  _kKBLayoutTypeUnknown,
  _kKBLayoutTypeANSI,
  _kKBLayoutTypeISO,
  _kKBLayoutTypeJIS,
];

class _KBImage extends StatelessWidget {
  final String kbLayoutType;
  final double imageWidth;
  final RxString chosenType;
  const _KBImage({
    Key? key,
    required this.kbLayoutType,
    required this.imageWidth,
    required this.chosenType,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      return Container(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(_kBorderRadius),
          border: Border.all(
            color: chosenType.value == kbLayoutType
                ? _kImageBorderColor
                : Colors.transparent,
            width: _kImageBoarderWidth,
          ),
        ),
        margin: EdgeInsets.symmetric(
          horizontal: _kImageMarginHorizontal,
          vertical: _kImageMarginVertical,
        ),
        padding: EdgeInsets.all(_kImagePaddingWidth),
        child: SvgPicture.asset(
          'assets/${_kKBLayoutImageMap[kbLayoutType] ?? ""}.svg',
          width: imageWidth -
              _kImageMarginHorizontal * 2 -
              _kImagePaddingWidth * 2 -
              _kImageBoarderWidth * 2,
        ),
      );
    });
  }
}

class _KBChooser extends StatelessWidget {
  final String kbLayoutType;
  final double imageWidth;
  final RxString chosenType;
  final KBChosenCallback cb;
  const _KBChooser({
    Key? key,
    required this.kbLayoutType,
    required this.imageWidth,
    required this.chosenType,
    required this.cb,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    onChanged(String? v) async {
      if (v != null) {
        if (await cb(v)) {
          chosenType.value = v;
        }
      }
    }

    return Column(
      children: [
        TextButton(
          onPressed: () {
            onChanged(kbLayoutType);
          },
          child: _KBImage(
            kbLayoutType: kbLayoutType,
            imageWidth: imageWidth,
            chosenType: chosenType,
          ),
          style: TextButton.styleFrom(padding: EdgeInsets.zero),
        ),
        TextButton(
          child: Row(
            children: [
              Obx(() => Radio(
                    splashRadius: 0,
                    value: kbLayoutType,
                    groupValue: chosenType.value,
                    onChanged: onChanged,
                  )),
              Text(kbLayoutType),
            ],
          ),
          onPressed: () {
            onChanged(kbLayoutType);
          },
        ),
      ],
    );
  }
}

class KBLayoutTypeChooser extends StatelessWidget {
  final RxString chosenType;
  final double width;
  final double height;
  final double dividerWidth;
  final KBChosenCallback cb;
  KBLayoutTypeChooser({
    Key? key,
    required this.chosenType,
    required this.width,
    required this.height,
    required this.dividerWidth,
    required this.cb,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final imageWidth = width / 2 - dividerWidth;
    return SizedBox(
      width: width,
      height: height,
      child: Center(
        child: Wrap(
          children: _kKBLayoutTypes
              .map((kbLayoutType) => SizedBox(
                    width: imageWidth,
                    child: _KBChooser(
                      kbLayoutType: kbLayoutType,
                      imageWidth: imageWidth,
                      chosenType: chosenType,
                      cb: cb,
                    ),
                  ))
              .toList(),
        ),
      ),
    );
  }
}

RxString KBLayoutType = ''.obs;

String getLocalPlatformForKBLayoutType(String peerPlatform) {
  String localPlatform = '';
  if (peerPlatform != kPeerPlatformMacOS) {
    return localPlatform;
  }

  if (isWindows) {
    localPlatform = kPeerPlatformWindows;
  } else if (isLinux) {
    localPlatform = kPeerPlatformLinux;
  } else if (isMacOS) {
    localPlatform = kPeerPlatformMacOS;
  } else if (isWebOnWindows || isWebOnLinux) {
    localPlatform = kPeerPlatformWebDesktop;
  }
  return localPlatform;
}

showKBLayoutTypeChooserIfNeeded(
  String peerPlatform,
  OverlayDialogManager dialogManager,
) async {
  final localPlatform = getLocalPlatformForKBLayoutType(peerPlatform);
  if (localPlatform == '') {
    return;
  }
  KBLayoutType.value = bind.getLocalKbLayoutType();
  if (KBLayoutType.value == _kKBLayoutTypeNotISO) {
    await bind.setLocalKbLayoutType(kbLayoutType: _kKBLayoutTypeANSI);
    KBLayoutType.value = bind.getLocalKbLayoutType();
  }
  if (_kKBLayoutTypes.contains(KBLayoutType.value)) {
    return;
  }
  showKBLayoutTypeChooser(localPlatform, dialogManager);
}

showKBLayoutTypeChooser(
  String localPlatform,
  OverlayDialogManager dialogManager,
) {
  dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title:
          Text('${translate('Select local keyboard type')} ($localPlatform)'),
      content: KBLayoutTypeChooser(
          chosenType: KBLayoutType,
          width: 360,
          height: 320,
          dividerWidth: 4.0,
          cb: (String v) async {
            await bind.setLocalKbLayoutType(kbLayoutType: v);
            KBLayoutType.value = bind.getLocalKbLayoutType();
            return v == KBLayoutType.value;
          }),
      actions: [dialogButton('Close', onPressed: close)],
      onCancel: close,
    );
  });
}
