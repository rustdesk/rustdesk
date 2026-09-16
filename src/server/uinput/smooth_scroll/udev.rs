use std::{io, path::Path};

const SYS_CLASS_INPUT_PATH: &str = "/sys/class/input";
const UDEV_DATA_PATH: &str = "/run/udev/data";

pub(super) fn is_classified_as_touchpad(event_path: &Path) -> io::Result<bool> {
    let event_name = validated_event_name(event_path)?;
    let device_number_path = Path::new(SYS_CLASS_INPUT_PATH).join(event_name).join("dev");
    let Some(device_number) = read_optional_text(&device_number_path)? else {
        return Ok(false);
    };
    let udev_path = Path::new(UDEV_DATA_PATH).join(format!("c{}", device_number.trim()));
    let Some(udev_data) = read_optional_text(&udev_path)? else {
        return Ok(false);
    };
    Ok(udev_data_is_touchpad(&udev_data))
}

fn validated_event_name(event_path: &Path) -> io::Result<&str> {
    let event_name = event_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid uinput event path"))?;
    let suffix = event_name.strip_prefix("event").unwrap_or_default();
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected uinput event path: {}", event_path.display()),
        ));
    }
    Ok(event_name)
}

fn read_optional_text(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn udev_data_is_touchpad(data: &str) -> bool {
    data.lines().any(|line| line == "E:ID_INPUT_TOUCHPAD=1")
}
