import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_svg/flutter_svg.dart';

/// Official Tefway wordmark, integrated at build time from `assets/logo.svg`.
const String kTefwayLogoAsset = 'assets/tefway_logo.svg';

const String _kMasterProfile = 'master';
const String _kClientProfile = 'client';

/// Which product this binary is. Baked in by the Rust build (`TEFWAY_PROFILE`)
/// and reported through a builtin option, so the UI cannot disagree with the
/// enforcement layer or be switched by the user.
String tefwayProfile() => bind.mainGetBuildinOption(key: 'tefway-profile');

bool isTefwayMaster() => tefwayProfile() == _kMasterProfile;

bool isTefwayClient() => tefwayProfile() == _kClientProfile;

bool isTefwayBuild() => isTefwayMaster() || isTefwayClient();

String tefwayProductName() {
  if (isTefwayMaster()) return 'Tefway Remote Master';
  if (isTefwayClient()) return 'Tefway Remote Client';
  return 'RustDesk';
}

String tefwayProfileDescription() {
  if (isTefwayMaster()) return 'Estação do técnico';
  if (isTefwayClient()) return 'Estação do cliente';
  return '';
}

/// Product name plus the role this build plays.
class TefwayProfileBadge extends StatelessWidget {
  const TefwayProfileBadge({super.key});

  @override
  Widget build(BuildContext context) {
    if (!isTefwayBuild()) return const SizedBox.shrink();
    final theme = Theme.of(context);
    final description = tefwayProfileDescription();
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            tefwayProductName(),
            textAlign: TextAlign.center,
            style: theme.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
          if (description.isNotEmpty)
            Text(
              description,
              textAlign: TextAlign.center,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.textTheme.bodySmall?.color?.withOpacity(0.7),
              ),
            ),
        ],
      ),
    );
  }
}

class TefwayLogo extends StatelessWidget {
  const TefwayLogo({super.key, this.height = 64, this.maxWidth = 300});

  final double height;
  final double maxWidth;

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth, maxHeight: height),
      child: SvgPicture.asset(
        kTefwayLogoAsset,
        fit: BoxFit.contain,
        placeholderBuilder: (context) => const SizedBox.shrink(),
      ),
    );
  }
}
