import 'package:auto_size_text/auto_size_text.dart';
import 'package:flutter/material.dart';

import '../../common.dart';

Widget getConnectionPageTitle(BuildContext context, bool isWeb) {
  return Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(
        'CONNECT',
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: MyTheme.accent,
              fontSize: 10,
              fontWeight: FontWeight.w700,
              letterSpacing: 1.5,
            ),
      ),
      const SizedBox(height: 6),
      Row(
        children: [
          Expanded(
            child: AutoSizeText(
              translate('Connect to a device'),
              maxLines: 1,
              style: Theme.of(context)
                  .textTheme
                  .titleLarge
                  ?.merge(const TextStyle(height: 1)),
            ),
          ),
          Tooltip(
            waitDuration: const Duration(milliseconds: 300),
            message: translate(isWeb ? "web_id_input_tip" : "id_input_tip"),
            child: Icon(
              Icons.help_outline_outlined,
              size: 16,
              color: Theme.of(context)
                  .textTheme
                  .titleLarge
                  ?.color
                  ?.withOpacity(0.5),
            ),
          ),
        ],
      ),
      const SizedBox(height: 5),
      Text(
        translate('Enter a device ID or choose one from your devices.'),
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context)
                  .textTheme
                  .bodySmall
                  ?.color
                  ?.withOpacity(0.65),
            ),
      ),
    ],
  );
}
