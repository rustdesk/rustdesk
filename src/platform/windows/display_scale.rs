use crate::platform::display_scale::{token, validate, Display, State, STALE, UNSUPPORTED};
use hbb_common::{bail, ResultType};
use std::{
    mem::{size_of, zeroed},
    ptr::null_mut,
    sync::OnceLock,
};
use winapi::{
    shared::windef::{HMONITOR, POINT},
    um::{
        libloaderapi::{GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_SEARCH_SYSTEM32},
        shellscalingapi::{MDT_EFFECTIVE_DPI, MONITOR_DPI_TYPE},
        wingdi::*,
        winuser::{MonitorFromPoint, MONITOR_DEFAULTTONULL},
    },
};

// The DPI packets are private extensions of DisplayConfig. Reject unknown ranges
// instead of guessing a DPI index on a Windows/driver version with a different ABI.
// https://github.com/lihas/windows-DPI-scaling-sample/blob/master/DPIHelper/DpiHelper.h
#[repr(C)]
struct DpiGet {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    min: i32,
    current: i32,
    max: i32,
}

#[repr(C)]
struct DpiSet {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    relative: i32,
}

const _: [(); 32] = [(); size_of::<DpiGet>()];
const _: [(); 24] = [(); size_of::<DpiSet>()];

#[link(name = "user32")]
extern "system" {
    fn GetDisplayConfigBufferSizes(flags: u32, paths: *mut u32, modes: *mut u32) -> i32;
    fn QueryDisplayConfig(
        flags: u32,
        paths_len: *mut u32,
        paths: *mut DISPLAYCONFIG_PATH_INFO,
        modes_len: *mut u32,
        modes: *mut DISPLAYCONFIG_MODE_INFO,
        topology: *mut u32,
    ) -> i32;
    fn DisplayConfigGetDeviceInfo(header: *mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32;
    fn DisplayConfigSetDeviceInfo(header: *const DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32;
}

const LEVELS: &[u32] = &[100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

fn check(code: i32) -> ResultType<()> {
    if code != 0 {
        bail!(
            "Failed to change display settings: {}",
            std::io::Error::from_raw_os_error(code)
        );
    }
    Ok(())
}

fn paths() -> ResultType<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>)> {
    for _ in 0..3 {
        let (mut np, mut nm) = (0, 0);
        unsafe {
            check(GetDisplayConfigBufferSizes(
                QDC_ONLY_ACTIVE_PATHS,
                &mut np,
                &mut nm,
            ))?;
        }
        if np == 0 || np > 128 || nm > 1024 {
            bail!(UNSUPPORTED);
        }
        let mut paths = vec![unsafe { zeroed() }; np as usize];
        let mut modes = vec![unsafe { zeroed() }; nm as usize];
        let code = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut np,
                paths.as_mut_ptr(),
                &mut nm,
                modes.as_mut_ptr(),
                null_mut(),
            )
        };
        if code == 122 {
            continue;
        } // Topology can change between the two calls.
        check(code)?;
        paths.truncate(np as usize);
        modes.truncate(nm as usize);
        return Ok((paths, modes));
    }
    bail!(STALE)
}

fn effective_dpi(monitor: HMONITOR) -> ResultType<(u32, u32)> {
    type GetDpiForMonitor =
        unsafe extern "system" fn(HMONITOR, MONITOR_DPI_TYPE, *mut u32, *mut u32) -> i32;
    static GET_DPI: OnceLock<Option<GetDpiForMonitor>> = OnceLock::new();
    // Shcore is absent before Windows 8.1. Keep the system DLL loaded for the
    // lifetime of the cached function without introducing a startup dependency.
    let Some(get_dpi) = GET_DPI.get_or_init(|| unsafe {
        let module = LoadLibraryExA(
            b"Shcore.dll\0".as_ptr() as _,
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if module.is_null() {
            return None;
        }
        let function = GetProcAddress(module, b"GetDpiForMonitor\0".as_ptr() as _);
        (!function.is_null()).then(|| std::mem::transmute::<_, GetDpiForMonitor>(function))
    }) else {
        bail!(UNSUPPORTED);
    };
    let (mut dx, mut dy) = (0, 0);
    if monitor.is_null() || unsafe { get_dpi(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) } != 0 {
        bail!(UNSUPPORTED);
    }
    Ok((dx, dy))
}

fn snapshot(name: &str) -> ResultType<(State, DpiGet)> {
    let (paths, modes) = paths()?;
    let mut matches = Vec::new();
    for path in &paths {
        let source = &path.sourceInfo;
        let mut packet: DISPLAYCONFIG_SOURCE_DEVICE_NAME = unsafe { zeroed() };
        packet.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
            _type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as _,
            adapterId: source.adapterId,
            id: source.id,
        };
        unsafe {
            check(DisplayConfigGetDeviceInfo(&mut packet.header))?;
        }
        let end = packet
            .viewGdiDeviceName
            .iter()
            .position(|v| *v == 0)
            .unwrap_or(packet.viewGdiDeviceName.len());
        if String::from_utf16_lossy(&packet.viewGdiDeviceName[..end]) == name {
            matches.push(path);
        }
    }
    // Clone targets share one DPI source; do not silently change another display.
    if matches.len() != 1 {
        bail!(UNSUPPORTED);
    }
    let path = matches[0];
    let source = &path.sourceInfo;
    let mut dpi = DpiGet {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            _type: (-3i32) as u32,
            size: size_of::<DpiGet>() as _,
            adapterId: source.adapterId,
            id: source.id,
        },
        min: 0,
        current: 0,
        max: 0,
    };
    unsafe {
        check(DisplayConfigGetDeviceInfo(&mut dpi.header))?;
    }
    let (recommended, current, last) = indices(dpi.min, dpi.current, dpi.max)?;
    let Some(mode) = modes
        .get(source.modeInfoIdx as usize)
        .filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE)
    else {
        bail!(UNSUPPORTED);
    };
    let mode = unsafe { mode.u.sourceMode() };
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: mode.position.x,
                y: mode.position.y,
            },
            MONITOR_DEFAULTTONULL,
        )
    };
    let (dx, dy) = effective_dpi(monitor)?;
    if dx != dy || dx * 100 != LEVELS[current] * 96 {
        // Custom global DPI and an unaware process do not use these per-source levels.
        bail!(UNSUPPORTED);
    }
    let mut target: DISPLAYCONFIG_TARGET_DEVICE_NAME = unsafe { zeroed() };
    target.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
        _type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as _,
        adapterId: path.targetInfo.adapterId,
        id: path.targetInfo.id,
    };
    unsafe {
        check(DisplayConfigGetDeviceInfo(&mut target.header))?;
    }
    let state = State {
        percent: LEVELS[current] as f64,
        custom: None,
        recommended: Some(LEVELS[recommended] as f64),
        options: LEVELS[..=last].iter().map(|v| *v as f64).collect(),
        token: token((
            name,
            &target.monitorDevicePath[..],
            source.adapterId.HighPart,
            source.adapterId.LowPart,
            source.id,
            mode.width,
            mode.height,
            mode.position.x,
            mode.position.y,
            dpi.min,
            dpi.current,
            dpi.max,
        )),
    };
    Ok((state, dpi))
}

fn indices(min: i32, current: i32, max: i32) -> ResultType<(usize, usize, usize)> {
    let recommended = -i64::from(min);
    let current = recommended + i64::from(current);
    let last = recommended + i64::from(max);
    if min > 0 || max < 0 || recommended > last || last >= LEVELS.len() as i64 {
        bail!(UNSUPPORTED);
    }
    // Windows retains the requested level when a new mode lowers the allowed
    // range. The effective DPI is capped until that mode supports it again.
    Ok((
        recommended as usize,
        current.clamp(0, last) as usize,
        last as usize,
    ))
}

pub fn read(display: &Display) -> ResultType<State> {
    Ok(snapshot(&display.name)?.0)
}

pub fn apply(display: &Display, percent: f64, expected: &str) -> ResultType<()> {
    let (state, dpi) = snapshot(&display.name)?;
    validate(&state, percent, expected)?;
    let Some(index) = LEVELS.iter().position(|v| *v as f64 == percent) else {
        bail!(UNSUPPORTED);
    };
    let relative = index as i32 + dpi.min;
    // A lower-resolution mode can cap the effective DPI while retaining a
    // higher requested level. An explicit selection must replace that level.
    if dpi.current == relative {
        return Ok(());
    }
    let packet = DpiSet {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            _type: (-4i32) as u32,
            size: size_of::<DpiSet>() as _,
            ..dpi.header
        },
        relative,
    };
    unsafe { check(DisplayConfigSetDeviceInfo(&packet.header)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_dpi_ranges() {
        assert_eq!(indices(-2, 1, 3).unwrap(), (2, 3, 5));
        for values in [(1, 1, 2), (-2, 0, -1), (i32::MIN, 0, i32::MAX), (-2, 0, 20)] {
            assert!(indices(values.0, values.1, values.2).is_err());
        }
    }

    #[test]
    fn retained_scale_is_limited_to_the_current_mode() {
        assert_eq!(indices(0, 3, 1).unwrap(), (0, 1, 1));
        assert_eq!(indices(-2, -3, 3).unwrap(), (2, 0, 5));
        assert_eq!(indices(-2, 4, 3).unwrap(), (2, 5, 5));
        assert_eq!(indices(0, 3, 3).unwrap(), (0, 3, 3));
    }
}
