import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../model.dart';

class Display extends StatelessWidget {
  final String peerId;
  final LocationModel locationModel;

  Display({
    Key? key,
    required this.peerId,
    required this.locationModel,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: locationModel,
      child: Consumer<LocationModel>(builder: (context, model, child) {
        return Column(
          children: [],
        );
      }),
    );
  }
}
