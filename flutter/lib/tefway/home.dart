// Entry point for the Tefway home surface of each profile.

import 'package:flutter/material.dart';

import 'branding.dart';
import 'pages/client_home_page.dart';
import 'pages/master_home_page.dart';

class TefwayHomePage extends StatelessWidget {
  const TefwayHomePage({super.key});

  @override
  Widget build(BuildContext context) {
    if (isTefwayMaster()) return const TefwayMasterHomePage();
    if (isTefwayClient()) return const TefwayClientHomePage();
    return const SizedBox.shrink();
  }
}
