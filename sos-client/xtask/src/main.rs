//! `cargo xtask` —— sos-client 的发布构建助手。
//!
//! 编译 → 复制到 `dist/` → UPX 压缩副本 → `upx -t` 回读校验 → 体积对比。
//!
//! # 为什么不用 `build.rs`
//!
//! 有两个硬性原因，不是风格偏好：
//!
//! 1. **`build.rs` 在链接之前运行**，那时 `target/release/rustdesk-sos.exe`
//!    还不存在，没有任何东西可压。
//! 2. **cargo 会缓存 build script**。源码没变时 build script 根本不会重新
//!    执行，于是「构建产物」会在某些增量编译里悄悄变成未压缩版本，
//!    而调用方毫不知情。
//!
//! xtask 模式（一个零依赖的小 crate + cargo 别名）是 Rust 社区处理这类
//! 「构建后处理」的标准做法：它由 cargo 启动，拿到真实路径，
//! 在产物确实存在之后才动手。
//!
//! # 为什么不原地压缩 `target/release/rustdesk-sos.exe`
//!
//! 因为那会破坏 cargo 的增量编译判据。cargo 用产物 mtime 判断「是否需要
//! 重新链接」：被 UPX 改写过的 exe 每次都比依赖新，于是**每一次**
//! `cargo build` 都会重新链接（多花几十秒），而脚本还得额外维护一套
//! 「这个文件到底压过没有」的判断（`upx -l` 探一次）以及配套的
//! `AlreadyPackedException` 容错。
//!
//! 改成「复制出去再压」之后，这些复杂性全部消失：
//!
//! - `target/release/` 永远是 cargo 的原样输出，增量编译正常工作；
//! - `dist/` 每次都是「刚从 target 拷来的未压缩文件 + 压一遍」，覆盖即可；
//! - 不存在「已压缩则跳过」这种分支，行为对使用者完全可预期。
//!
//! # 用法
//!
//! ```text
//! cd sos-client
//! cargo xtask                    # 编译 + 压缩，产出 dist/rustdesk-sos.exe
//! cargo xtask --no-upx           # 只复制不压缩
//! cargo xtask --out-dir out      # 换个输出目录
//! cargo xtask -- --offline       # `--` 之后的参数原样透传给 cargo build
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

// ── 配置 ──

/// 产物名（不含平台后缀）
const BIN_NAME: &str = "rustdesk-sos";

/// 默认输出目录（相对 sos-client 根目录）
const DEFAULT_OUT_DIR: &str = "dist";

/// 本地放置的 UPX 位置（相对 sos-client 根目录）
///
/// 该目录**不入库**（见 `sos-client/.gitignore`）—— UPX 是第三方二进制，
/// 体积大且有单独的许可证。这里只是为了方便：把 upx.exe 手动放进
/// `tools/upx/` 就能被自动发现，不必额外设环境变量。
const VENDORED_UPX_DIR: &str = "tools/upx";

/// 自动补全时查询 UPX 最新 release 的 API 端点。
///
/// # 版本策略：始终跟随最新 release
///
/// **不在源码里钉死版本号或哈希**。理由：UPX 发布安全修复之后，钉死的
/// 版本会让所有构建长期停留在有已知问题的旧二进制上，而升级需要改代码、
/// 走一遍宿主项目的发布流程。第三方工具的安全更新不该绑在宿主项目的
/// 发布节奏上。
///
/// # 完整性怎么保证
///
/// 每次下载都向 GitHub API 取该 release 的 asset sha256（`digest` 字段）
/// 并**强制校验**，不存在「跳过校验」的分支。这能拦住传输损坏、CDN /
/// 第三方镜像篡改。挡不住的是 GitHub 账号本身被攻破 —— 那种情况下攻击者
/// 可以同时替换资产与 digest。这是「自动获取最新二进制」这条路无法回避的
/// 信任根。
///
/// # 需要脱离网络或彻底自控时
///
/// 用 `--upx-path` 指向本地已有的 upx.exe，或用 `--upx-url` 指向镜像
/// （下载后仍会用 GitHub 公布的 digest 校验，确保与官方资产字节一致）。
const UPX_RELEASES_API: &str = "https://api.github.com/repos/upx/upx/releases/latest";

// ── 入口 ──

/// 把控制台输出代码页临时切到 UTF-8，退出时恢复。
///
/// Rust 的 `println!` 写的是 UTF-8 字节，而中文 Windows 的控制台默认是
/// GBK(936)，于是中文全变成乱码。这是「程序输出 UTF-8、终端按 OEM 代码页解」
/// 的经典错配 —— 之前的 `.bat` 之所以要求纯 ASCII，也是同一个原因。
///
/// 这里只在确实有控制台时切换（重定向到文件时 `GetConsoleOutputCP` 返回 0，
/// 那种情况下 UTF-8 本来就是对的，不该动），并在进程退出时还原，
/// 不把父终端的代码页搞乱。
#[cfg(windows)]
#[derive(Default)]
struct Utf8Console {
    prev_out: u32,
    prev_in: u32,
    active: bool,
}

#[cfg(windows)]
impl Utf8Console {
    fn enable() -> Self {
        extern "system" {
            fn GetConsoleOutputCP() -> u32;
            fn GetConsoleCP() -> u32;
            fn SetConsoleOutputCP(cp: u32) -> i32;
            fn SetConsoleCP(cp: u32) -> i32;
        }
        const CP_UTF8: u32 = 65001;

        let prev_out = unsafe { GetConsoleOutputCP() };
        if prev_out == 0 {
            // 没有控制台（输出被重定向），保持原样
            return Self::default();
        }
        let prev_in = unsafe { GetConsoleCP() };
        unsafe {
            SetConsoleOutputCP(CP_UTF8);
            SetConsoleCP(CP_UTF8);
        }
        Self {
            prev_out,
            prev_in,
            active: true,
        }
    }
}

#[cfg(windows)]
impl Drop for Utf8Console {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        extern "system" {
            fn SetConsoleOutputCP(cp: u32) -> i32;
            fn SetConsoleCP(cp: u32) -> i32;
        }
        unsafe {
            SetConsoleOutputCP(self.prev_out);
            SetConsoleCP(self.prev_in);
        }
    }
}

#[cfg(not(windows))]
struct Utf8Console;

#[cfg(not(windows))]
impl Utf8Console {
    fn enable() -> Self {
        Utf8Console
    }
}

fn main() -> ExitCode {
    let _console = Utf8Console::enable();

    let opts = match Options::parse(std::env::args().skip(1)) {
        Ok(Some(o)) => o,
        // --help
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误：{e}");
            eprintln!();
            print_help();
            return ExitCode::from(2);
        }
    };

    match run(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("错误：{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(opts: Options) -> Result<ExitCode, String> {
    let root = project_root()?;
    let built = root.join("target").join("release").join(exe_file_name());

    build(&root, &opts)?;

    // target/release 是 cargo 的原样输出，必须**原封不动地留着**，
    // 否则下次 cargo 会认为产物过期而重新链接。
    if !built.is_file() {
        return Err(format!("构建成功但找不到产物：{}", built.display()));
    }
    if !pe_is_valid(&built)? {
        return Err(format!(
            "产物不是合法的 PE 文件，构建可能被中断：{}",
            built.display()
        ));
    }
    let raw = file_len(&built)?;
    step("产物");
    println!("{}  ({})", built.display(), fmt_size(raw));

    // 交付物目录
    let out_dir = match &opts.out_dir {
        Some(d) => {
            let p = PathBuf::from(d);
            if p.is_absolute() {
                p
            } else {
                root.join(p)
            }
        }
        None => root.join(DEFAULT_OUT_DIR),
    };
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("创建输出目录 {} 失败：{e}", out_dir.display()))?;
    let out = out_dir.join(exe_file_name());

    step("复制到输出目录");
    std::fs::copy(&built, &out).map_err(|e| format!("复制到 {} 失败：{e}", out.display()))?;
    println!("{}", out.display());

    if opts.no_upx {
        step("跳过 UPX（--no-upx）");
        println!("未压缩产物：{}  ({})", out.display(), fmt_size(raw));
        return Ok(ExitCode::SUCCESS);
    }

    step("UPX");
    // 显式指定了路径就用它，找不到直接报错。
    // 不能退回「自动下载」—— 用户指定路径的意图就是「用那一份」
    // （可能是内网授权的、或某个特定版本），静默换成下载的会误导。
    if let Some(p) = opts.upx_path.as_deref() {
        let path = Path::new(p);
        if !path.is_file() {
            return Err(format!(
                "--upx-path 指定的文件不存在：{p}\n\
                 想让它自动下载的话，去掉 --upx-path 即可（或用 --upx-url 指定镜像）。"
            ));
        }
    }
    let upx = match find_upx(&root, opts.upx_path.as_deref()) {
        Some(p) => p,
        None => {
            // 找不到时先尝试自动补全。它把 upx.exe 落到 tools/upx/ 下，
            // 之后就变成「本机副本」被 find_upx 直接命中，不再重复下载。
            if opts.no_upx_fetch || env_flag("SOS_NO_UPX_FETCH") {
                print_install_hints(&out);
                return Ok(ExitCode::FAILURE);
            }
            step("自动获取 UPX");
            match fetch_upx(&root, opts.upx_url.as_deref()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("自动获取失败：{e}");
                    eprintln!();
                    print_install_hints(&out);
                    eprintln!("自动下载可以用 --no-upx-fetch 关掉，或 --upx-url 指向内网镜像。");
                    return Ok(ExitCode::FAILURE);
                }
            }
        }
    };
    println!("UPX：{}", upx.display());
    if let Some(v) = upx_version(&upx) {
        println!("版本：{v}");
    }

    let mut args: Vec<String> = Vec::new();
    match opts.level {
        Level::Best => args.push("--best".into()),
        Level::Normal => {}
        Level::Fast => args.push("--fast".into()),
    }
    if opts.lzma {
        args.push("--lzma".into());
    }
    args.push("--no-progress".into());
    args.push(out.display().to_string());

    println!("upx {}", args.join(" "));
    let status = Command::new(&upx)
        .args(&args)
        .status()
        .map_err(|e| format!("无法启动 upx：{e}"))?;
    if !status.success() {
        return Err(format!("UPX 压缩失败（{status}）"));
    }

    step("校验");
    if !pe_is_valid(&out)? {
        return Err(format!("压缩后不再是合法 PE：{}", out.display()));
    }
    println!("PE 文件头：正常");

    let status = Command::new(&upx)
        .arg("-t")
        .arg(&out)
        .status()
        .map_err(|e| format!("无法启动 upx -t：{e}"))?;
    if !status.success() {
        return Err(format!("upx -t 回读校验失败（{status}），产物可能已损坏"));
    }
    println!("upx -t 完整性：正常");

    let new = file_len(&out)?;
    let ratio = if raw > 0 {
        100.0 * new as f64 / raw as f64
    } else {
        100.0
    };

    step("完成");
    println!("压缩前：{}", fmt_size(raw));
    println!("压缩后：{}", fmt_size(new));
    println!(
        "比例：  {ratio:.1}%（节省 {}）",
        fmt_size(raw.saturating_sub(new))
    );
    println!();
    println!("交付物：{}", out.display());
    println!(
        "cargo 原始产物（未压缩，保留给增量编译）：{}",
        built.display()
    );
    Ok(ExitCode::SUCCESS)
}

// ── 构建 ──

fn build(root: &Path, opts: &Options) -> Result<(), String> {
    step("cargo build --release");
    let manifest = root.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!("找不到 Cargo.toml：{}", manifest.display()));
    }

    let mut argv: Vec<String> = vec![
        "build".into(),
        "--release".into(),
        "--manifest-path".into(),
        manifest.display().to_string(),
    ];
    argv.extend(opts.cargo_args.iter().cloned());
    println!("cargo {}", argv.join(" "));

    let status = Command::new(cargo_exe())
        .args(&argv)
        .current_dir(root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("无法启动 cargo：{e}"))?;
    if !status.success() {
        return Err(format!("cargo build 失败（{status}）"));
    }
    Ok(())
}

/// 优先用 cargo 自己提供的路径，避免 PATH 里混入另一个工具链
fn cargo_exe() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn project_root() -> Result<PathBuf, String> {
    // xtask 的 manifest 目录是 sos-client/xtask，上一级就是 sos-client
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| format!("无法推断项目根目录，manifest={}", manifest_dir.display()))
}

fn exe_file_name() -> String {
    if cfg!(windows) {
        format!("{BIN_NAME}.exe")
    } else {
        BIN_NAME.to_string()
    }
}

// ── PE / 文件工具 ──

/// 只做文件头校验：`MZ` + PE 签名。用来确认 UPX 没把产物写坏。
fn pe_is_valid(path: &Path) -> Result<bool, String> {
    let data = std::fs::read(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    Ok(pe_has_valid_header(&data))
}

/// 纯函数版本，便于单测
fn pe_has_valid_header(data: &[u8]) -> bool {
    if data.len() < 0x40 || data[0] != b'M' || data[1] != b'Z' {
        return false;
    }
    let off = i32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]);
    if off < 0 {
        return false;
    }
    let off = off as usize;
    // e_lfanew 来自文件内容，可能是任意值 —— 必须防越界
    off.checked_add(4)
        .filter(|end| *end <= data.len())
        .is_some_and(|end| &data[off..end] == b"PE\0\0")
}

fn file_len(path: &Path) -> Result<u64, String> {
    std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| format!("读取 {} 元数据失败：{e}", path.display()))
}

fn fmt_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let f = bytes as f64;
    if f >= MB {
        format!("{:.2} MB", f / MB)
    } else if f >= KB {
        format!("{:.1} KB", f / KB)
    } else {
        format!("{bytes} B")
    }
}

fn step(title: &str) {
    println!();
    println!("=== {title} ===");
}

// ── UPX ──

fn print_install_hints(out: &Path) {
    println!("找不到 upx.exe，{} 保持未压缩。", out.display());
    println!();
    println!("安装方式（任选其一）：");
    println!("  winget install UPX.UPX");
    println!("  choco install upx");
    println!("  scoop install upx");
    println!("或从 https://github.com/upx/upx/releases 下载后：");
    println!("  cargo xtask --upx-path C:\\path\\to\\upx.exe");
    println!("  设置环境变量 UPX_PATH 后自动识别");
    println!("  把 upx.exe 放进本机的 {VENDORED_UPX_DIR}/ 也会被自动发现");
    println!();
    println!("只想编译不要压缩：cargo xtask --no-upx");
}

/// 读一个「设了就算真」的环境变量。
///
/// 空串、`0`、`false` 都算未设置 —— 有些 CI 会把未定义的变量导成空串。
fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let v = v.trim();
            !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false")
        }
        Err(_) => false,
    }
}

/// 一次解析出来的 UPX 发行信息。
struct ReleaseInfo {
    /// 不带 `v` 前缀的版本号，例如 `5.3.0`
    version: String,
    /// win64 zip 的直链
    url: String,
    /// zip 的 sha256（小写 hex，不含 `sha256:` 前缀）
    sha256: String,
}

/// 决定这次要下载哪个版本：**永远是 GitHub 上的最新 release**。
///
/// `url_override`（来自 `--upx-url`）只替换**下载位置**，版本与 sha256
/// 仍取自 GitHub API —— 这样镜像或代理下的文件会被校验成与官方资产
/// 字节一致，不会因为「换了下载源」而失去完整性保障。
fn resolve_upx_release(url_override: Option<&str>) -> Result<ReleaseInfo, String> {
    let mut info = probe_latest_release()?;
    if let Some(u) = url_override {
        let u = u.trim();
        if u.is_empty() {
            return Err("--upx-url 是空的".into());
        }
        info.url = u.to_string();
    }
    Ok(info)
}

/// 问 GitHub API 要最新 release 的版本 / win64 直链 / sha256。
///
/// 用 PowerShell 解析 JSON，而不是自己写一个解析器，也不是引入 serde：
/// xtask 保持零依赖（否则 `cargo xtask` 上线前要先编译几十个 crate），
/// 而 PowerShell 自带可靠的 JSON 实现。这是一次性开销（仅在没有本地副本时
/// 发生），200ms 左右可以接受。
fn probe_latest_release() -> Result<ReleaseInfo, String> {
    // 只取三个字段，用换行分隔，保持输出极小且好解析
    let script = format!(
        "$ProgressPreference='SilentlyContinue'; \
         [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; \
         $h=@{{'User-Agent'='rustdesk-sos-xtask';'Accept'='application/vnd.github+json'}}; \
         $r=Invoke-RestMethod -Uri '{api}' -Headers $h -TimeoutSec 30; \
         $a=$r.assets | Where-Object {{ $_.name -like '*win64.zip' }} | Select-Object -First 1; \
         if (-not $a) {{ throw 'no win64 asset in latest release' }}; \
         Write-Output $r.tag_name; \
         Write-Output $a.browser_download_url; \
         Write-Output $a.digest",
        api = ps_quote(UPX_RELEASES_API)
    );

    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("无法启动 powershell 查询 UPX 最新版本：{e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = msg.trim();
        return Err(format!(
            "查询 UPX 最新版本失败：{}\n\
             离线环境请用 --upx-path 指定本地 upx.exe，或 --no-upx-fetch 直接不压缩。",
            if msg.is_empty() {
                "（无输出）"
            } else {
                msg
            }
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_release_probe(&text).ok_or_else(|| {
        format!(
            "无法从 GitHub API 响应中解析出 UPX 发行信息：\n{}\n\
             （GitHub 可能没有为该资产公开 digest；此时请用 --upx-path 指定本地副本）",
            text.trim()
        )
    })
}

/// 解析 [`probe_latest_release`] 的 PowerShell 输出（三行：tag / url / digest）。
///
/// 独立成纯函数便于单测 —— 这里做的每一项校验都是「宁可失败也不要
/// 用未经验证的二进制」。
fn parse_release_probe(text: &str) -> Option<ReleaseInfo> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());

    let tag = lines.next()?;
    let url = lines.next()?;
    let digest = lines.next()?;

    let version = tag.strip_prefix('v').unwrap_or(tag).to_string();
    // 版本号必须像版本号：首字符是数字，只含数字/点
    if !version.starts_with(|c: char| c.is_ascii_digit())
        || !version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_ascii_alphabetic())
    {
        return None;
    }
    if !url.starts_with("https://") || !url.contains("win64") {
        return None;
    }
    let sha256 = digest.strip_prefix("sha256:")?.to_ascii_lowercase();
    if !is_sha256_hex(&sha256) {
        return None;
    }
    Some(ReleaseInfo {
        version,
        url: url.to_string(),
        sha256,
    })
}

/// 是否是合法的 sha256（64 位小写/大写十六进制）
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// 下载缺失的 UPX 到 `tools/upx/`，返回可执行的 `upx.exe` 路径。
///
/// 版本与 sha256 每次都从 GitHub API 现取（见 [`UPX_RELEASES_API`] 的说明），
/// 下载后**强制**校验，没有「跳过校验」的分支。`--upx-url` 只改变下载位置，
/// 校验值仍来自 GitHub —— 镜像给的文件必须与官方资产字节一致。
fn fetch_upx(root: &Path, url_override: Option<&str>) -> Result<PathBuf, String> {
    let release = resolve_upx_release(url_override)?;

    let dest_dir = root.join(VENDORED_UPX_DIR);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("创建 {} 失败：{e}", dest_dir.display()))?;

    println!("版本：{}（GitHub 最新）", release.version);
    println!("地址：{}", release.url);

    // 先下到 .part，校验通过再改名 —— 否则中途失败会在缓存目录留下
    // 一个半截文件，下次 find_upx 可能把它当成「已有副本」。
    let zip = dest_dir.join(format!("upx-{}-win64.zip", release.version));
    let part = zip.with_extension("zip.part");
    let _ = std::fs::remove_file(&part);

    println!("下载中……");
    http_download(&release.url, &part)?;

    let len = std::fs::metadata(&part)
        .map_err(|e| format!("读取下载文件失败：{e}"))?
        .len();
    if len == 0 {
        let _ = std::fs::remove_file(&part);
        return Err("下载到 0 字节文件".into());
    }
    println!("已下载：{}", fmt_size(len));

    let actual = sha256_of(&part)?;
    if actual != release.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "sha256 与 GitHub 公布值不符，已丢弃下载\n  期望：{}\n  实际：{actual}\n\
             若用的是 --upx-url 镜像，说明该文件与官方资产不一致。",
            release.sha256
        ));
    }
    println!("sha256：匹配");

    std::fs::rename(&part, &zip).map_err(|e| format!("重命名下载文件失败：{e}"))?;

    println!("解压中……");
    unzip_archive(&zip, &dest_dir)?;

    // 解压后不假设目录名（zip 里是 upx-<ver>-win64/），直接递归找 exe。
    // 目录里可能有旧版本，按版本优先取刚下载的那个。
    let mut found = Vec::new();
    collect_upx(&dest_dir, &mut found);
    found.sort();
    let exe = found
        .iter()
        .rev()
        .find(|p| p.to_string_lossy().contains(&release.version))
        .or_else(|| found.last())
        .cloned()
        .ok_or_else(|| format!("解压后没找到 upx.exe（{}）", dest_dir.display()))?;

    // 能跑起来才算真的可用：杀软可能把解压出来的 exe 直接隔离掉。
    match upx_version(&exe) {
        Some(v) => println!("已就绪：{}（{v}）", exe.display()),
        None => {
            return Err(format!(
                "{} 无法运行（可能被杀软拦截，或不是有效的 win64 二进制）",
                exe.display()
            ))
        }
    }

    // 压缩包留着没意义（exe 已经出来了），删掉省空间。
    let _ = std::fs::remove_file(&zip);

    Ok(exe)
}

/// 下载 `url` 到 `dest`。
///
/// 优先用系统自带的 `curl.exe`（Windows 10 1803+）：它有真实的超时和重试，
/// 而 `Invoke-WebRequest` 在慢链路上容易一直挂着。两者都不引入 Rust 依赖 ——
/// xtask 刻意保持零依赖，否则 `cargo xtask` 自己就要先编译几十个 crate。
fn http_download(url: &str, dest: &Path) -> Result<(), String> {
    let dest_str = dest.to_string_lossy().to_string();

    let curl = Command::new("curl.exe")
        .args([
            "-fsSL", // 静默 + 失败时返回非零（-f 是关键：否则 404 也会写出响应体）
            "--retry",
            "2",
            "--connect-timeout",
            "15",
            "--max-time",
            "300",
            "-o",
            &dest_str,
            url,
        ])
        .status();
    match curl {
        Ok(s) if s.success() => return Ok(()),
        Ok(s) => {
            return Err(format!(
                "curl 下载失败（{s}）—— 检查网络/代理，或改用 --upx-url 指定镜像"
            ))
        }
        Err(_) => {
            // curl.exe 不存在（老系统 / 精简镜像），退到 PowerShell
        }
    }

    let script = format!(
        "$ProgressPreference='SilentlyContinue'; \
         [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; \
         Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
        ps_quote(url),
        ps_quote(&dest_str)
    );
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("既没有 curl.exe 也无法启动 powershell：{e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = msg.trim();
        return Err(format!(
            "PowerShell 下载失败：{}",
            if msg.is_empty() {
                "（无输出）"
            } else {
                msg
            }
        ));
    }
    Ok(())
}

/// 转义 PowerShell 单引号字符串里的单引号（`'` → `''`）。
fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

/// 计算文件的 sha256（小写 hex）。
///
/// 用 `certutil`：所有 Windows 都自带，不必为了一个哈希引入 sha2 crate。
fn sha256_of(path: &Path) -> Result<String, String> {
    let out = Command::new("certutil")
        .arg("-hashfile")
        .arg(path)
        .arg("SHA256")
        .output()
        .map_err(|e| format!("无法启动 certutil：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "certutil 失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_certutil_sha256(&text).ok_or_else(|| format!("无法从 certutil 输出解析 sha256：{text}"))
}

/// 从 `certutil -hashfile <f> SHA256` 的输出里取出哈希。
///
/// 输出形如（中英文一致的结构，只有首行提示语不同）：
///
/// ```text
/// SHA256 的 C:\x.zip 哈希:
/// d127996c76615e431be810e9c8d2910dbe3dd400600c5c0c50276fc5a17a9705
/// CertUtil: -hashfile 命令成功完成。
/// ```
///
/// 不靠提示语定位 —— 那是本地化的。只认「去掉空格后正好是 64 位十六进制」
/// 的那一行，并且要求有且只有一行（多行说明输出异常，宁可报错也不猜）。
fn parse_certutil_sha256(text: &str) -> Option<String> {
    let mut hits: Vec<String> = text
        .lines()
        .map(|l| l.trim().replace(' ', ""))
        .filter(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|l| l.to_ascii_lowercase())
        .collect();
    if hits.len() == 1 {
        hits.pop()
    } else {
        None
    }
}

/// 解压 zip 到 `dest`。
///
/// 用 `tar.exe -xf`（Windows 10 1803+ 自带的 bsdtar，能读 zip），
/// 比 `Expand-Archive` 快得多，老系统上再退回去。
fn unzip_archive(zip: &Path, dest: &Path) -> Result<(), String> {
    let tar = Command::new("tar.exe")
        .arg("-xf")
        .arg(zip)
        .arg("-C")
        .arg(dest)
        .status();
    match tar {
        Ok(s) if s.success() => return Ok(()),
        Ok(s) => return Err(format!("tar 解压失败（{s}）")),
        Err(_) => {
            // tar.exe 不存在，退到 PowerShell
        }
    }

    let script = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        ps_quote(&zip.to_string_lossy()),
        ps_quote(&dest.to_string_lossy())
    );
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("既没有 tar.exe 也无法启动 powershell：{e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = msg.trim();
        return Err(format!(
            "解压失败：{}",
            if msg.is_empty() {
                "（无输出）"
            } else {
                msg
            }
        ));
    }
    Ok(())
}

/// 查找顺序：显式参数 > `UPX_PATH` > 本机 tools/upx 副本 > PATH > 常见安装位置
fn find_upx(root: &Path, explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }

    if let Ok(p) = std::env::var("UPX_PATH") {
        if !p.is_empty() {
            let p = PathBuf::from(p);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 本机副本（不入库）：tools/upx/**/upx.exe，多个版本时取排序后的最后一个
    let mut found = Vec::new();
    collect_upx(&root.join(VENDORED_UPX_DIR), &mut found);
    found.sort();
    if let Some(p) = found.pop() {
        return Some(p);
    }

    // PATH：能跑起来就算有
    if Command::new("upx")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(PathBuf::from("upx"));
    }

    // 常见安装位置。环境变量可能不存在，逐个判空 —— 用空串 join 会得到
    // 相对路径 `upx\upx.exe`，那会被误判成「找到」。
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "LOCALAPPDATA",
        "ChocolateyInstall",
    ] {
        if let Ok(base) = std::env::var(var) {
            if !base.is_empty() {
                let base = PathBuf::from(base);
                candidates.push(base.join("upx").join("upx.exe"));
                candidates.push(base.join("bin").join("upx.exe"));
            }
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        if !home.is_empty() {
            candidates.push(
                PathBuf::from(home)
                    .join("scoop")
                    .join("shims")
                    .join("upx.exe"),
            );
        }
    }
    candidates.push(PathBuf::from(r"C:\upx\upx.exe"));

    candidates.into_iter().find(|p| p.is_file())
}

fn collect_upx(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_upx(&path, out);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("upx.exe"))
        {
            out.push(path);
        }
    }
}

fn upx_version(upx: &Path) -> Option<String> {
    let out = Command::new(upx)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().next().map(|l| l.trim().to_string())
}

// ── 参数 ──

#[derive(Clone, Copy)]
enum Level {
    Best,
    Normal,
    Fast,
}

struct Options {
    no_upx: bool,
    /// 禁止自动下载（离线环境 / 想自己控制用哪一份）
    no_upx_fetch: bool,
    lzma: bool,
    level: Level,
    out_dir: Option<String>,
    upx_path: Option<String>,
    /// 自定义下载地址（内网镜像 / 代理）
    upx_url: Option<String>,
    /// `--` 之后的内容，原样交给 cargo build
    cargo_args: Vec<String>,
}

impl Options {
    /// 返回 `Ok(None)` 表示只需要打印帮助
    fn parse<I: Iterator<Item = String>>(mut args: I) -> Result<Option<Self>, String> {
        let mut opts = Options {
            no_upx: false,
            no_upx_fetch: false,
            lzma: true,
            level: Level::Best,
            out_dir: None,
            upx_path: None,
            upx_url: None,
            cargo_args: Vec::new(),
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                // 子命令：与缺省行为相同；显式接受以免被当成未知参数
                "release" | "build" => {}
                "--" => {
                    opts.cargo_args.extend(args);
                    break;
                }
                "-h" | "--help" | "help" => {
                    print_help();
                    return Ok(None);
                }
                "--no-upx" => opts.no_upx = true,
                "--no-upx-fetch" => opts.no_upx_fetch = true,
                "--no-lzma" => opts.lzma = false,
                "--level" => {
                    let v = args
                        .next()
                        .ok_or("--level 需要一个值（best/normal/fast）")?;
                    opts.level = parse_level(&v)?;
                }
                "--out-dir" => {
                    let v = args.next().ok_or("--out-dir 需要一个路径")?;
                    opts.out_dir = Some(v);
                }
                "--upx-path" => {
                    let v = args.next().ok_or("--upx-path 需要一个路径")?;
                    opts.upx_path = Some(v);
                }
                "--upx-url" => {
                    let v = args.next().ok_or("--upx-url 需要一个 URL")?;
                    opts.upx_url = Some(v);
                }
                other => {
                    // 支持 --level=fast 这种写法
                    if let Some(v) = other.strip_prefix("--level=") {
                        opts.level = parse_level(v)?;
                    } else if let Some(v) = other.strip_prefix("--out-dir=") {
                        opts.out_dir = Some(v.to_string());
                    } else if let Some(v) = other.strip_prefix("--upx-path=") {
                        opts.upx_path = Some(v.to_string());
                    } else if let Some(v) = other.strip_prefix("--upx-url=") {
                        opts.upx_url = Some(v.to_string());
                    } else {
                        return Err(format!("未知参数：{other}"));
                    }
                }
            }
        }
        Ok(Some(opts))
    }
}

fn parse_level(v: &str) -> Result<Level, String> {
    match v {
        "best" => Ok(Level::Best),
        "normal" => Ok(Level::Normal),
        "fast" => Ok(Level::Fast),
        other => Err(format!("无效的 --level：{other}（可选 best/normal/fast）")),
    }
}

fn print_help() {
    println!(
        "\
cargo xtask —— sos-client 发布构建（编译 + 复制 + UPX 压缩 + 校验）

用法：
  cargo xtask [选项] [-- <cargo build 的额外参数>]

选项：
  --no-upx                只复制不压缩
  --no-upx-fetch          找不到 UPX 时不自动下载（离线/受控环境）
  --no-lzma               不用 LZMA（体积略大、解压稍快）
  --level <best|normal|fast>
                          压缩等级，默认 best
  --out-dir <PATH>        交付物目录，默认 {DEFAULT_OUT_DIR}
  --upx-path <PATH>       指定 upx.exe
  --upx-url <URL>         从指定地址下载 UPX（内网镜像/代理）
  -h, --help              显示本帮助

产物：
  target/release/{BIN_NAME}.exe   cargo 原样输出，**不压缩**（保证增量编译正常）
  {DEFAULT_OUT_DIR}/{BIN_NAME}.exe        最终交付物（UPX 压缩后）

UPX 查找顺序：
  --upx-path > 环境变量 UPX_PATH > 本机 {VENDORED_UPX_DIR}/ 下的副本
  > PATH > 常见安装位置 > 自动下载

自动下载：
  都找不到时，向 GitHub API 查询 UPX **最新 release** 并下载到 {VENDORED_UPX_DIR}/，
  用该 release 的官方 sha256 强制校验；之后一直复用这份本地副本。
  版本与哈希都不在本项目里钉死 —— UPX 的安全修复能直接在下次构建生效。
  关掉：--no-upx-fetch 或环境变量 SOS_NO_UPX_FETCH=1
  镜像：--upx-url <URL>（仍用 GitHub 公布的 sha256 校验，确保与官方资产一致）
  完全离线：--upx-path 指定本地 upx.exe

示例：
  cargo xtask
  cargo xtask --no-upx
  cargo xtask --no-upx-fetch
  cargo xtask --out-dir out -- --offline

注意：
  签名必须在压缩之后做 —— UPX 会改写 PE 布局，先签名会被破坏。
  UPX 壳 + 远程控制类程序容易被杀软启发式误报，
  部署环境严格时请用 --no-upx 出未压缩版本。"
    );
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用的假哈希（**不是**任何真实产物的指纹，仅为断言可读性）。
    const DEMO_SHA256: &str = "d127996c76615e431be810e9c8d2910dbe3dd400600c5c0c50276fc5a17a9705";

    #[test]
    fn pe_header_must_start_with_mz() {
        assert!(!pe_has_valid_header(b""));
        assert!(!pe_has_valid_header(&[0u8; 64]));
        let mut data = vec![0u8; 64];
        data[0] = b'M';
        data[1] = b'Z';
        // e_lfanew = 0 —— 指向文件头本身，必然不是 PE 签名
        assert!(!pe_has_valid_header(&data));
    }

    #[test]
    fn pe_header_accepts_well_formed_signature() {
        let mut data = vec![0u8; 0x80];
        data[0] = b'M';
        data[1] = b'Z';
        // e_lfanew = 0x40
        data[0x3C..0x40].copy_from_slice(&0x40i32.to_le_bytes());
        data[0x40..0x44].copy_from_slice(b"PE\0\0");
        assert!(pe_has_valid_header(&data));
    }

    #[test]
    fn pe_header_rejects_out_of_range_lfanew() {
        let mut data = vec![0u8; 0x80];
        data[0] = b'M';
        data[1] = b'Z';
        // e_lfanew 指向文件末尾之外，不能让切片越界
        data[0x3C..0x40].copy_from_slice(&0x7FFFi32.to_le_bytes());
        assert!(!pe_has_valid_header(&data));
        // 负数同理
        data[0x3C..0x40].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(!pe_has_valid_header(&data));
    }

    #[test]
    fn size_formatting_uses_binary_units() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(1023), "1023 B");
        assert_eq!(fmt_size(1024), "1.0 KB");
        assert_eq!(fmt_size(1024 * 1024), "1.00 MB");
        assert_eq!(fmt_size(9_187_328), "8.76 MB");
    }

    #[test]
    fn level_parsing_rejects_unknown() {
        assert!(parse_level("best").is_ok());
        assert!(parse_level("normal").is_ok());
        assert!(parse_level("fast").is_ok());
        assert!(parse_level("ultra").is_err());
        assert!(parse_level("").is_err());
    }

    #[test]
    fn options_default_to_best_with_lzma_and_dist() {
        let o = Options::parse(std::iter::empty()).unwrap().unwrap();
        assert!(!o.no_upx);
        assert!(o.lzma);
        assert!(matches!(o.level, Level::Best));
        assert!(o.out_dir.is_none(), "默认走 DEFAULT_OUT_DIR");
        assert!(o.cargo_args.is_empty());
    }

    #[test]
    fn passthrough_after_double_dash_goes_to_cargo() {
        let args: Vec<String> = ["--no-upx", "--", "--offline", "--locked"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = Options::parse(args.into_iter()).unwrap().unwrap();
        assert!(o.no_upx);
        assert_eq!(o.cargo_args, vec!["--offline", "--locked"]);
    }

    #[test]
    fn level_accepts_both_separated_and_equals_forms() {
        let a: Vec<String> = ["--level", "fast"].iter().map(|s| s.to_string()).collect();
        let o = Options::parse(a.into_iter()).unwrap().unwrap();
        assert!(matches!(o.level, Level::Fast));

        let b: Vec<String> = ["--level=normal"].iter().map(|s| s.to_string()).collect();
        let o = Options::parse(b.into_iter()).unwrap().unwrap();
        assert!(matches!(o.level, Level::Normal));
    }

    #[test]
    fn out_dir_accepts_both_forms() {
        let a: Vec<String> = ["--out-dir", "out"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            Options::parse(a.into_iter())
                .unwrap()
                .unwrap()
                .out_dir
                .as_deref(),
            Some("out")
        );

        let b: Vec<String> = ["--out-dir=build/dist"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            Options::parse(b.into_iter())
                .unwrap()
                .unwrap()
                .out_dir
                .as_deref(),
            Some("build/dist")
        );
    }

    #[test]
    fn unknown_flag_is_reported_not_silently_ignored() {
        let args: Vec<String> = ["--nope"].iter().map(|s| s.to_string()).collect();
        assert!(Options::parse(args.into_iter()).is_err());
    }

    #[test]
    fn upx_fetch_is_on_by_default() {
        let o = Options::parse(std::iter::empty()).unwrap().unwrap();
        assert!(
            !o.no_upx_fetch,
            "默认应允许自动补全，否则离线以外都要手动装 UPX"
        );
        assert!(o.upx_url.is_none());
    }

    #[test]
    fn no_upx_fetch_disables_auto_download() {
        let args: Vec<String> = ["--no-upx-fetch"].iter().map(|s| s.to_string()).collect();
        let o = Options::parse(args.into_iter()).unwrap().unwrap();
        assert!(o.no_upx_fetch);
    }

    #[test]
    fn upx_url_accepts_both_forms() {
        let a: Vec<String> = ["--upx-url", "https://mirror/upx.zip"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            Options::parse(a.into_iter())
                .unwrap()
                .unwrap()
                .upx_url
                .as_deref(),
            Some("https://mirror/upx.zip")
        );

        let b: Vec<String> = ["--upx-url=https://mirror/u.zip"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            Options::parse(b.into_iter())
                .unwrap()
                .unwrap()
                .upx_url
                .as_deref(),
            Some("https://mirror/u.zip")
        );
    }

    #[test]
    fn release_probe_parses_plain_three_line_output() {
        let text = "v5.3.0\n\
                    https://github.com/upx/upx/releases/download/v5.3.0/upx-5.3.0-win64.zip\n\
                    sha256:d127996c76615e431be810e9c8d2910dbe3dd400600c5c0c50276fc5a17a9705\n";
        let r = parse_release_probe(text).expect("应能解析");
        assert_eq!(r.version, "5.3.0", "tag 的 v 前缀要去掉");
        assert!(r.url.ends_with("upx-5.3.0-win64.zip"));
        assert_eq!(r.sha256, DEMO_SHA256);
    }

    #[test]
    fn release_probe_requires_sha256_digest() {
        // 没有 digest（老 API 或非 GitHub 源）→ 宁可失败，也不下没有校验值的文件
        let text = "v5.3.0\n\
                    https://github.com/upx/upx/releases/download/v5.3.0/upx-5.3.0-win64.zip\n";
        assert!(parse_release_probe(text).is_none());
        // digest 为空行同样拒绝
        let empty = "v5.3.0\nhttps://x/upx-5.3.0-win64.zip\n\n";
        assert!(parse_release_probe(empty).is_none());
    }

    #[test]
    fn release_probe_rejects_non_https_or_non_win64() {
        let sha = format!("sha256:{DEMO_SHA256}");
        for url in [
            "http://example.com/upx-5.3.0-win64.zip", // 非 https
            "https://example.com/upx-linux.tar.xz",   // 不是 win64
        ] {
            let text = format!("v5.3.0\n{url}\n{sha}\n");
            assert!(parse_release_probe(&text).is_none(), "应拒绝 {url}");
        }
    }

    #[test]
    fn release_probe_rejects_bogus_tag() {
        let sha = format!("sha256:{DEMO_SHA256}");
        for tag in ["not-a-version", "", "vv5.3.0", "-5.3.0"] {
            let text = format!("{tag}\nhttps://x/upx-5.3.0-win64.zip\n{sha}\n");
            assert!(parse_release_probe(&text).is_none(), "tag {tag:?} 不该通过");
        }
    }

    #[test]
    fn sha256_validation_checks_length_and_charset() {
        assert!(is_sha256_hex(&"a".repeat(64)));
        assert!(is_sha256_hex(&"A".repeat(64)));
        assert!(!is_sha256_hex(&"a".repeat(63)));
        assert!(!is_sha256_hex(&"a".repeat(65)));
        assert!(!is_sha256_hex(&"g".repeat(64)));
        assert!(!is_sha256_hex(""));
    }

    #[test]
    fn certutil_hash_is_extracted_from_localized_output() {
        // 中文系统
        let zh = format!(
            "SHA256 的 C:\\tmp\\u.zip 哈希:\r\n{DEMO_SHA256}\r\n\
             CertUtil: -hashfile 命令成功完成。\r\n"
        );
        assert_eq!(parse_certutil_sha256(&zh).as_deref(), Some(DEMO_SHA256));

        // 英文系统：提示语不同，哈希行结构相同
        let en = "SHA256 hash of C:\\tmp\\u.zip:\r\n\
                  0000000000000000000000000000000000000000000000000000000000000001\r\n\
                  CertUtil: -hashfile command completed successfully.\r\n";
        assert_eq!(parse_certutil_sha256(en), Some("0".repeat(63) + "1"));
    }

    #[test]
    fn certutil_hash_accepts_space_separated_digits() {
        // 某些 certutil 版本把哈希按 4 位一组用空格分开（仍在同一行内）
        let text = "SHA256 hash of x:\r\n\
                    d127 996c 7661 5e43 1be8 10e9 c8d2 910d be3d d400 600c 5c0c 5027 6fc5 a17a 9705\r\n\
                    CertUtil: -hashfile command completed successfully.\r\n";
        assert_eq!(parse_certutil_sha256(text).as_deref(), Some(DEMO_SHA256));
    }

    #[test]
    fn certutil_hash_rejects_ambiguous_output() {
        // 没有 64 位 hex 行
        assert_eq!(parse_certutil_sha256(""), None);
        assert_eq!(parse_certutil_sha256("no hash here\n"), None);
        // 多行都像哈希 → 无法判断哪个是真的，宁可失败
        let two = format!("{h}\n{h}\n", h = "a".repeat(64));
        assert_eq!(parse_certutil_sha256(&two), None);
        // 长度对但不是十六进制
        assert_eq!(parse_certutil_sha256(&"z".repeat(64)), None);
    }

    #[test]
    fn ps_quote_escapes_single_quotes() {
        assert_eq!(ps_quote("plain"), "plain");
        assert_eq!(ps_quote(r"C:\it's"), r"C:\it''s");
    }
}
