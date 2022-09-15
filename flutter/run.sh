#!/usr/bin/env bash

dart pub global activate ffigen --version 5.0.1
flutter pub get
# call `flutter clean` if cargo build fails
cargo build --features flutter
flutter run $@
