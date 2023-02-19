#[cfg(target_os = "linux")]
pub mod linux;

use crate::{log, config::Config, ResultType};
use std::{collections::HashMap, process::{Command, exit}};

extern "C" fn breakdown_signal_handler(sig: i32) {
    let mut stack = vec![];
    backtrace::trace(|frame| {
        backtrace::resolve_frame(frame, |symbol| {
            if let Some(name) = symbol.name() {
                stack.push(name.to_string());
            }
        });
        true // keep going to the next frame
    });
    let mut info = String::default();
    if stack.iter().any(|s| {
        s.contains(&"nouveau_pushbuf_kick")
            || s.to_lowercase().contains("nvidia")
            || s.contains("gdk_window_end_draw_frame")
    }) {
        Config::set_option("allow-always-software-render".to_string(), "Y".to_string());
        info = "Always use software rendering will be set.".to_string();
        log::info!("{}", info);
    }
    log::error!(
        "Got signal {} and exit. stack:\n{}",
        sig,
        stack.join("\n").to_string()
    );
    if !info.is_empty() {
        system_message(
            "RustDesk",
            &format!("Got signal {} and exit.{}", sig, info),
            true,
        )
        .ok();
    }
    exit(0);
}

/// forever: may not work
pub fn system_message(title: &str, msg: &str, forever: bool) -> ResultType<()> {
    let cmds: HashMap<&str, Vec<&str>> = HashMap::from([
        ("notify-send", [title, msg].to_vec()),
        (
            "zenity",
            [
                "--info",
                "--timeout",
                if forever { "0" } else { "3" },
                "--title",
                title,
                "--text",
                msg,
            ]
            .to_vec(),
        ),
        ("kdialog", ["--title", title, "--msgbox", msg].to_vec()),
        (
            "xmessage",
            [
                "-center",
                "-timeout",
                if forever { "0" } else { "3" },
                title,
                msg,
            ]
            .to_vec(),
        ),
    ]);
    for (k, v) in cmds {
        if Command::new(k).args(v).spawn().is_ok() {
            return Ok(());
        }
    }
    crate::bail!("failed to post system message");
}

pub fn register_breakdown_handler() {
    unsafe {
        libc::signal(libc::SIGSEGV, breakdown_signal_handler as _);
    }
}
