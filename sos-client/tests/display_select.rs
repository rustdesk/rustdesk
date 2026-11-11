//! 显示器选择与分辨率切换逻辑。
//!
//! 生产问题（全部基于代码走查）：
//!
//! 1. `Misc::SwitchDisplay` **只打日志**，索引被丢弃；
//!    而 `CaptureState::new()` 硬编码 `displays.remove(0)` → 永远只采集第一块屏。
//! 2. 尺寸检测用 `GetSystemMetrics(SM_CXSCREEN)` —— 那是**主显示器**的尺寸。
//!    采集副屏时改副屏分辨率，这里检测不到变化，编码器尺寸停在旧值。
//! 3. 改分辨率时先 `request_refresh()` 再 `change_resolution()`，
//!    而重建只要 33ms、改名要 100-500ms → 白闪一次再重建第二次。
//! 4. `handle_change_resolution` 用 `display_service::try_get_displays()` 的顺序，
//!    而控制端发来的索引来自 `scrap::Display::all()` 的顺序 → 可能改错屏。
//!
//! 运行：cargo test --release --test display_select -- --nocapture

#![cfg(windows)]

#[path = "../src/sos_display.rs"]
mod sos_display;

use sos_display::{resolve_index, size_by_name, size_changed};

#[test]
fn single_display_always_resolves_to_zero() {
    assert_eq!(resolve_index(0, 1), Some(0));
    assert_eq!(resolve_index(1, 1), Some(0), "越界应回退到 0");
    assert_eq!(resolve_index(7, 1), Some(0));
}

#[test]
fn valid_index_is_preserved() {
    assert_eq!(resolve_index(0, 3), Some(0));
    assert_eq!(resolve_index(1, 3), Some(1));
    assert_eq!(resolve_index(2, 3), Some(2));
}

#[test]
fn out_of_range_falls_back_to_zero() {
    // 显示器被拔掉后索引失效：回退到 0，而不是永久失败
    assert_eq!(resolve_index(5, 3), Some(0));
    assert_eq!(resolve_index(i32::MAX, 3), Some(0));
}

#[test]
fn negative_index_is_clamped() {
    // 协议字段是 int32，异常值不能让程序 panic
    assert_eq!(resolve_index(-1, 2), Some(0));
    assert_eq!(resolve_index(i32::MIN, 2), Some(0));
}

#[test]
fn no_display_is_none() {
    assert_eq!(resolve_index(0, 0), None);
    assert_eq!(resolve_index(5, 0), None);
}

#[test]
fn unchanged_size_does_not_trigger_rebuild() {
    assert!(!size_changed((1920, 1080), Some((1920, 1080))));
}

#[test]
fn changed_size_triggers_rebuild() {
    assert!(size_changed((1920, 1080), Some((1280, 720))));
    assert!(size_changed((1920, 1080), Some((1920, 1200))));
    assert!(size_changed((1920, 1080), Some((2560, 1080))));
}

/// 查询失败（显示器正在切换模式、驱动瞬时不可用）时**不能**触发重建，
/// 否则每次查询失败都会重建一次编码器，形成死循环。
#[test]
fn failed_query_does_not_trigger_rebuild() {
    assert!(!size_changed((1920, 1080), None));
}

/// **核心回归测试**：按设备名查到的尺寸必须与 `Display::width()/height()`
/// 完全一致。
///
/// 这条差点被 `EnumDisplaySettingsW` 破坏：那家伙返回**物理像素**，
/// 而 `Display::width()` 返回的是与进程 DPI 感知模式一致的逻辑像素
/// （实测 125% 缩放下 2880 vs 2304）。用错数据源会让尺寸检测恒定判「变了」，
/// 编码器每秒重建一次。
#[test]
fn size_by_name_matches_display_enumeration() {
    let Ok(displays) = scrap::Display::all() else {
        println!("[skip] no display");
        return;
    };
    assert!(!displays.is_empty(), "至少应有一块显示器");
    for (i, d) in displays.iter().enumerate() {
        let name = d.name();
        let by_enum = (d.width() as usize, d.height() as usize);
        let by_query = size_by_name(&name);
        println!(
            "#{} '{}' Display::all()={:?} size_by_name={:?}",
            i, name, by_enum, by_query
        );
        assert_eq!(
            by_query,
            Some(by_enum),
            "显示器 #{} '{}' 的两种查询结果必须一致，否则会无限重建",
            i,
            name
        );
    }
}

/// 显示器名字不能是空串，否则按名查询会退化成主显示器，
/// 多显示器下就分辨不出来了。
#[test]
fn display_names_are_non_empty() {
    let Ok(displays) = scrap::Display::all() else {
        println!("[skip] no display");
        return;
    };
    for (i, d) in displays.iter().enumerate() {
        let name = d.name();
        println!("#{} -> {:?}", i, name);
        assert!(!name.is_empty(), "显示器 #{} 的设备名为空", i);
    }
}

/// 未知名字必须回退到主显示器而不是返回 `None` —— UAC 帧没有显示器名字。
#[test]
fn unknown_name_falls_back_to_primary() {
    let primary = sos_display::primary_size();
    let unknown = size_by_name("\\\\.\\NOPE999");
    println!("primary={:?} unknown={:?}", primary, unknown);
    assert_eq!(unknown, primary);
}
