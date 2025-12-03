# Quick Android APK Build Guide for RustDesk

## 📱 Способ 1: Быстро через GitHub Actions (РЕКОМЕНДУЕТСЯ)

### Шаги:
1. **Загрузи коммит на GitHub:**
```bash
git add .
git commit -m "Add auto-update and --service by default"
git push origin master
```

2. **Запусти GitHub Actions вручную:**
```bash
gh workflow run flutter-build.yml -f upload-artifact=true -f upload-tag=v1.4.4-dev
```

3. **Скачай APK:**
   - Перейди на https://github.com/rustdesk/rustdesk/actions
   - Найди последний запуск `Build the flutter version...`
   - Перейди на вкладку "Artifacts"
   - Скачай `rustdesk-1.4.4-aarch64.apk` (arm64)

---

## 🐧 Способ 2: Локально на Linux (WSL2 или Ubuntu)

### Быстрая установка (10-15 минут):

```bash
cd ~/rustdesk_src/rustdesk

# 1. Установи зависимости
sudo apt-get update && sudo apt-get install -y \
  clang cmake curl gcc-multilib g++ g++-multilib libunwind-dev \
  ninja-build openjdk-17-jdk-headless pkg-config wget

# 2. Установи Rust + cargo-ndk
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
cargo install cargo-ndk

# 3. Установи Android NDK r27c
mkdir -p $HOME/android
cd $HOME/android
wget https://dl.google.com/android/repository/android-ndk-r27c-linux.zip
unzip android-ndk-r27c-linux.zip
export ANDROID_NDK_HOME=$HOME/android/android-ndk-r27c

# 4. Установи Flutter
git clone https://github.com/flutter/flutter.git -b stable --depth 1
export PATH="$PATH:$HOME/flutter/bin"
flutter doctor

# 5. Собери Android Native Libraries
cd ~/rustdesk_src/rustdesk
./flutter/ndk_arm64.sh  # для ARM64
./flutter/ndk_arm.sh    # для ARM (опционально)

# 6. Собери APK
cd flutter
MODE=release ./build_android.sh

# APK будет в: build/app/outputs/flutter-apk/
```

### Результат:
```
build/app/outputs/flutter-apk/app-release.apk          (universal)
build/app/outputs/flutter-apk/app-arm64-v8a-release.apk (ARM64)
build/app/outputs/flutter-apk/app-armeabi-v7a-release.apk (ARM 32-bit)
```

---

## 🪟 Способ 3: WSL2 на Windows (ПРОСТОЙ)

```powershell
# На Windows PowerShell
wsl --install Ubuntu-24.04

# После загрузки в WSL:
wsl
cd /mnt/d/rustdesk_src/rustdesk
bash -c "
  sudo apt-get update && sudo apt-get install -y curl git build-essential
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source \$HOME/.cargo/env
  cargo install cargo-ndk
  
  mkdir -p \$HOME/android
  cd \$HOME/android
  wget https://dl.google.com/android/repository/android-ndk-r27c-linux.zip
  unzip android-ndk-r27c-linux.zip
  
  export ANDROID_NDK_HOME=\$HOME/android/android-ndk-r27c
  export PATH=\$HOME/flutter/bin:\$PATH
  
  cd /mnt/d/rustdesk_src/rustdesk/flutter
  MODE=release ./build_android.sh
"
```

---

## 📊 Сравнение методов

| Метод | Время | Сложность | Требования |
|-------|-------|-----------|-----------|
| **GitHub Actions** | 30-40 мин | ⭐ Очень просто | GitHub аккаунт |
| **WSL2** | 20-30 мин | ⭐⭐ Просто | Windows 10/11 |
| **Ubuntu Linux** | 15-20 мин | ⭐⭐⭐ Средне | Ubuntu VM или натив |
| **Docker** | 25-35 мин | ⭐⭐⭐ Средне | Docker desktop |

---

## 🔍 Проверка результата

После компиляции распакуй APK и проверь:
```bash
unzip app-release.apk
cat AndroidManifest.xml | grep -E "package|versionName"
```

Должно показать:
```
package="com.carriez.flutter_hbb"
android:versionName="1.4.4"
```

---

## ✅ Итоговые файлы

После успешной сборки получишь:
- ✅ `app-arm64-v8a-release.apk` — для большинства современных Android устройств (ARM64)
- ✅ `app-armeabi-v7a-release.apk` — для старых устройств (ARM 32-bit)
- ✅ `app-release.apk` — универсальный APK (содержит оба)

---

## 🐛 Решение проблем

**Ошибка: "Flutter not found"**
```bash
export PATH="$HOME/flutter/bin:$PATH"
```

**Ошибка: "Android NDK not found"**
```bash
export ANDROID_NDK_HOME=$HOME/android/android-ndk-r27c
```

**Ошибка: "cargo-ndk not found"**
```bash
cargo install cargo-ndk
```

**Ошибка при vcpkg**
```bash
cd ~/rustdesk_src/rustdesk
./flutter/build_android_deps.sh arm64-v8a
```

---

## 📝 Рекомендация

**Для быстрого теста:** Используй GitHub Actions (Способ 1) — просто push и жди 30-40 минут.

**Для регулярной разработки:** Используй WSL2 (Способ 3) — установи один раз, потом быстро компилируешь локально.
