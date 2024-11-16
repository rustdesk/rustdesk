#!/bin/bash

# patch often fails to apply on windows, use sed instead
# https://github.com/flutter/flutter/commit/b5847d364a26d727af58ab885a6123e0e5304b2b#diff-634a338bd9ed19b66a27beba35a8acf4defffd8beff256113e6811771a0c4821R543

SED_CMD="sed -i"

FILE="packages/flutter/lib/src/material/dropdown_menu.dart"

$SED_CMD '478s/late bool _enableFilter;/bool _enableFilter = false;/' "$FILE"

$SED_CMD '526a\
    if (oldWidget.enableFilter != widget.enableFilter) {\
      if (!widget.enableFilter) {\
        _enableFilter = false;\
      }\
    }' "$FILE"

$SED_CMD '670a\
              _enableFilter = false;' "$FILE"

$SED_CMD '743a\
    } else {\
      filteredEntries = widget.dropdownMenuEntries;' "$FILE"

echo "Modifications applied to $FILE"