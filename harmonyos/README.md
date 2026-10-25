# RustDesk 鸿蒙（HarmonyOS）主控端

原生 ArkTS 应用 + Rust 核心动态库的混合架构。Rust 会话引擎（协议、编解码、
文件传输）原样复用 `src/` 代码，交叉编译为 `aarch64-unknown-linux-ohos` 目标的
`librustdesk.so`；ArkTS 通过 NAPI（dlopen/dlsym）驱动它，负责 UI、渲染与输入。

FFI 契约（两侧唯一接口规范）：`docs/harmonyos-migration/ohos-controller-ffi.md`

```
harmonyos/
├── AppScope/               应用级配置（bundleName 等）
├── entry/                  ArkTS 模块
│   └── src/main/
│       ├── cpp/            NAPI 包装（dlopen librustdesk.so + RGBA→NativeWindow 渲染）
│       └── ets/            页面：Index（连接）/ RemotePage（远程桌面）/ FileTransferPage
└── client/                 Rust 侧构建
    ├── build-ohos-deps.sh  vcpkg 构建 libvpx/libyuv/aom（arm64-ohos triplet）
    ├── build-rust.sh       交叉编译 librustdesk.so
    ├── deps/               (生成) 预编译 C 库 include/lib
    └── out/                (生成) librustdesk.so 产物
```

## 构建步骤（Windows + DevEco Studio 26）

```bash
# 1. Rust ohos 目标（一次性）
rustup target add aarch64-unknown-linux-ohos

# 2. 预编译解码库（libvpx/libyuv/libaom，首次约 20-40 分钟）
bash harmonyos/client/build-ohos-deps.sh

# 3. 交叉编译 Rust 核心 → harmonyos/client/out/aarch64-ohos/librustdesk.so
bash harmonyos/client/build-rust.sh

# 4. 把 .so 放进 HAP 的 native 库目录
mkdir -p harmonyos/entry/libs/arm64-v8a
cp harmonyos/client/out/aarch64-ohos/librustdesk.so harmonyos/entry/libs/arm64-v8a/

# 5. 构建 HAP（命令行；或在 DevEco Studio 中打开 harmonyos/）
export DEVECO_SDK_HOME="D:/Huawei/DevEco Studio"
export PATH="$(cygpath -u "D:/Huawei/DevEco Studio/jbr/bin"):$PATH"   # PackageHap 需要 java
"D:/Huawei/DevEco Studio/tools/node/node.exe" \
  "D:/Huawei/DevEco Studio/tools/hvigor/bin/hvigorw.js" \
  --no-daemon assembleHap --mode module -p product=default -p buildMode=debug
# 产物: entry/build/default/outputs/default/entry-default-unsigned.hap
```

说明：

- NDK 自动从 DevEco Studio 安装目录检测（`D:/Huawei/DevEco Studio/...`），
  并在 `~/.rustdesk-ohos/ndk` 建无空格 junction（RUSTFLAGS 不能含空格路径）。
- `librustdesk.so` 在 ArkTS 侧是运行时 dlopen，缺失时应用不崩溃，仅提示错误；
  因此第 4 步可以晚于前端开发进行。
- 安装到真机/模拟器需要签名：在 DevEco Studio 的 File → Project Structure →
  Signing Configs 里勾选 Automatically generate signature 后重新构建。
- 主控端 M1 能力边界：VP8/VP9 软解观看、触控/软键盘输入、文件传输、
  剪贴板（客户端方向，文本）。音频播放、硬件解码、远端光标渲染、双指缩放
  尚未实现（见契约文档 §5）。
