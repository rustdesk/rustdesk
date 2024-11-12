// https://github.com/aarnt/qt-sudo
// Sometimes reboot is needed to refresh sudoers.

use crate::lang::translate;
use gtk::{glib, prelude::*};
use hbb_common::{
    anyhow::{bail, Error},
    log, ResultType,
};
use nix::{
    libc::{fcntl, kill},
    pty::{forkpty, ForkptyResult},
    sys::{
        signal::Signal,
        wait::{waitpid, WaitPidFlag},
    },
    unistd::{execvp, setsid, Pid},
};
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

const EXIT_CODE: i32 = -1;

enum Message {
    PasswordPrompt((String, bool)),
    Password((String, String)),
    ErrorDialog(String),
    Cancel,
    Exit(i32),
}

pub fn run(cmds: Vec<&str>) -> ResultType<()> {
    // rustdesk service kill `rustdesk --` processes
    let second_arg = std::env::args().nth(1).unwrap_or_default();
    let cmd_mode =
        second_arg.starts_with("--") && second_arg != "--tray" && second_arg != "--no-server";
    let mod_arg = if cmd_mode { "cmd" } else { "gui" };
    let mut args = vec!["-gtk-sudo", mod_arg];
    args.append(&mut cmds.clone());
    let mut child = crate::run_me(args)?;
    let exit_status = child.wait()?;
    if exit_status.success() {
        Ok(())
    } else {
        bail!("child exited with status: {:?}", exit_status);
    }
}

pub fn exec() {
    let mut args = vec![];
    for arg in std::env::args().skip(3) {
        args.push(arg);
    }
    let cmd_mode = std::env::args().nth(2) == Some("cmd".to_string());
    if cmd_mode {
        cmd(args);
    } else {
        ui(args);
    }
}

fn cmd(args: Vec<String>) {
    match unsafe { forkpty(None, None) } {
        Ok(forkpty_result) => match forkpty_result {
            ForkptyResult::Parent { child, master } => {
                if let Err(e) = cmd_parent(child, master) {
                    log::error!("Parent error: {:?}", e);
                    kill_child(child);
                    std::process::exit(EXIT_CODE);
                }
            }
            ForkptyResult::Child => {
                if let Err(e) = child(None, args) {
                    log::error!("Child error: {:?}", e);
                    std::process::exit(EXIT_CODE);
                }
            }
        },
        Err(err) => {
            log::error!("forkpty error: {:?}", err);
            std::process::exit(EXIT_CODE);
        }
    }
}

fn ui(args: Vec<String>) {
    // https://docs.gtk.org/gtk4/ctor.Application.new.html
    // https://docs.gtk.org/gio/type_func.Application.id_is_valid.html
    let application = gtk::Application::new(None, Default::default());

    let (tx_to_ui, rx_to_ui) = channel::<Message>();
    let (tx_from_ui, rx_from_ui) = channel::<Message>();

    let rx_to_ui = Arc::new(Mutex::new(rx_to_ui));
    let tx_from_ui = Arc::new(Mutex::new(tx_from_ui));

    let rx_to_ui_clone = rx_to_ui.clone();
    let tx_from_ui_clone = tx_from_ui.clone();

    let username = Arc::new(Mutex::new(crate::platform::get_active_username()));
    let username_clone = username.clone();

    application.connect_activate(glib::clone!(@weak application =>move |_| {
        let rx_to_ui = rx_to_ui_clone.clone();
        let tx_from_ui = tx_from_ui_clone.clone();
        let last_password = Arc::new(Mutex::new(String::new()));
        let username = username_clone.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            if let Ok(msg) = rx_to_ui.lock().unwrap().try_recv() {
                match msg {
                    Message::PasswordPrompt((err_msg, show_edit)) => {
                        let last_pwd = last_password.lock().unwrap().clone();
                        let username = username.lock().unwrap().clone();
                        if let Some((username, password)) = password_prompt(&username, &last_pwd, &err_msg, show_edit) {
                                *last_password.lock().unwrap() = password.clone();
                                if let Err(e) = tx_from_ui
                                    .lock()
                                    .unwrap()
                                    .send(Message::Password((username, password))) {
                                        error_dialog_and_exit(&format!("Channel error: {e:?}"), EXIT_CODE);
                                    }
                        } else {
                            if let Err(e) = tx_from_ui.lock().unwrap().send(Message::Cancel) {
                                error_dialog_and_exit(&format!("Channel error: {e:?}"), EXIT_CODE);
                            }
                        }
                    }
                    Message::ErrorDialog(err_msg) => {
                        error_dialog_and_exit(&err_msg, EXIT_CODE);
                    }
                    Message::Exit(code) => {
                        log::info!("Exit code: {}", code);
                        std::process::exit(code);
                    }
                    _ => {}
                }
            }
            glib::ControlFlow::Continue
        });
    }));

    let tx_to_ui_clone = tx_to_ui.clone();
    std::thread::spawn(move || {
        let acitve_user = crate::platform::get_active_username();
        let mut initial_password = None;
        if acitve_user != "root" {
            if let Err(e) = tx_to_ui_clone.send(Message::PasswordPrompt(("".to_string(), true))) {
                log::error!("Channel error: {e:?}");
                std::process::exit(EXIT_CODE);
            }
            match rx_from_ui.recv() {
                Ok(Message::Password((user, password))) => {
                    *username.lock().unwrap() = user;
                    initial_password = Some(password);
                }
                Ok(Message::Cancel) => {
                    log::info!("User canceled");
                    std::process::exit(EXIT_CODE);
                }
                _ => {
                    log::error!("Unexpected message");
                    std::process::exit(EXIT_CODE);
                }
            }
        }
        let username = username.lock().unwrap().clone();
        let su_user = if username == acitve_user {
            None
        } else {
            Some(username)
        };
        match unsafe { forkpty(None, None) } {
            Ok(forkpty_result) => match forkpty_result {
                ForkptyResult::Parent { child, master } => {
                    if let Err(e) = ui_parent(
                        child,
                        master,
                        tx_to_ui_clone,
                        rx_from_ui,
                        su_user.is_some(),
                        initial_password,
                    ) {
                        log::error!("Parent error: {:?}", e);
                        kill_child(child);
                        std::process::exit(EXIT_CODE);
                    }
                }
                ForkptyResult::Child => {
                    if let Err(e) = child(su_user, args) {
                        log::error!("Child error: {:?}", e);
                        std::process::exit(EXIT_CODE);
                    }
                }
            },
            Err(err) => {
                log::error!("forkpty error: {:?}", err);
                if let Err(e) =
                    tx_to_ui.send(Message::ErrorDialog(format!("Forkpty error: {:?}", err)))
                {
                    log::error!("Channel error: {e:?}");
                    std::process::exit(EXIT_CODE);
                }
            }
        }
    });

    let _holder = application.hold();
    let args: Vec<&str> = vec![];
    application.run_with_args(&args);
    log::debug!("exit from gtk::Application::run_with_args");
    std::process::exit(EXIT_CODE);
}

fn cmd_parent(child: Pid, master: OwnedFd) -> ResultType<()> {
    let raw_fd = master.as_raw_fd();
    if unsafe { fcntl(raw_fd, nix::libc::F_SETFL, nix::libc::O_NONBLOCK) } != 0 {
        let errno = std::io::Error::last_os_error();
        bail!("fcntl error: {errno:?}");
    }
    let mut file = unsafe { File::from_raw_fd(raw_fd) };
    let mut stdout = std::io::stdout();
    let stdin = std::io::stdin();
    let stdin_fd = stdin.as_raw_fd();
    let old_termios = termios::Termios::from_fd(stdin_fd)?;
    turn_off_echo(stdin_fd).ok();
    shutdown_hooks::add_shutdown_hook(turn_on_echo_shutdown_hook);
    let (tx, rx) = channel::<Vec<u8>>();
    std::thread::spawn(move || loop {
        let mut line = String::default();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                kill_child(child);
                break;
            }
            Ok(_) => {
                if let Err(e) = tx.send(line.as_bytes().to_vec()) {
                    log::error!("Channel error: {e:?}");
                    kill_child(child);
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                log::info!("Failed to read stdin: {e:?}");
                kill_child(child);
                break;
            }
        };
    });
    loop {
        let mut buf = [0; 1024];
        match file.read(&mut buf) {
            Ok(0) => {
                log::info!("read from child: EOF");
                break;
            }
            Ok(n) => {
                let buf = String::from_utf8_lossy(&buf[..n]).to_string();
                print!("{}", buf);
                if let Err(e) = stdout.flush() {
                    log::error!("flush failed: {e:?}");
                    kill_child(child);
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                // Child process is dead
                log::info!("Read child error: {:?}", e);
                break;
            }
        }
        match rx.try_recv() {
            Ok(v) => {
                if let Err(e) = file.write_all(&v) {
                    log::error!("write error: {e:?}");
                    kill_child(child);
                    break;
                }
            }
            Err(e) => match e {
                std::sync::mpsc::TryRecvError::Empty => {}
                std::sync::mpsc::TryRecvError::Disconnected => {
                    log::error!("receive error: {e:?}");
                    kill_child(child);
                    break;
                }
            },
        }
    }

    // Wait for child process
    let status = waitpid(child, None);
    log::info!("waitpid status: {:?}", status);
    let mut code = EXIT_CODE;
    match status {
        Ok(s) => match s {
            nix::sys::wait::WaitStatus::Exited(_pid, status) => {
                code = status;
            }
            _ => {}
        },
        Err(_) => {}
    }
    termios::tcsetattr(stdin_fd, termios::TCSANOW, &old_termios).ok();
    std::process::exit(code);
}

fn ui_parent(
    child: Pid,
    master: OwnedFd,
    tx_to_ui: Sender<Message>,
    rx_from_ui: Receiver<Message>,
    is_su: bool,
    initial_password: Option<String>,
) -> ResultType<()> {
    let mut initial_password = initial_password;
    let raw_fd = master.as_raw_fd();
    if unsafe { fcntl(raw_fd, nix::libc::F_SETFL, nix::libc::O_NONBLOCK) } != 0 {
        let errno = std::io::Error::last_os_error();
        tx_to_ui.send(Message::ErrorDialog(format!("fcntl error: {errno:?}")))?;
        bail!("fcntl error: {errno:?}");
    }
    let mut file = unsafe { File::from_raw_fd(raw_fd) };

    let mut first = initial_password.is_none();
    let mut su_password_sent = false;
    let mut saved_output = String::default();
    loop {
        let mut buf = [0; 1024];
        match file.read(&mut buf) {
            Ok(0) => {
                log::info!("read from child: EOF");
                break;
            }
            Ok(n) => {
                saved_output = String::default();
                let buf = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                let last_line = buf.lines().last().unwrap_or(&buf).trim().to_string();
                log::info!("read from child: {}", buf);

                if last_line.starts_with("sudo:") || last_line.starts_with("su:") {
                    if let Err(e) = tx_to_ui.send(Message::ErrorDialog(last_line)) {
                        log::error!("Channel error: {e:?}");
                        kill_child(child);
                    }
                    break;
                } else if last_line.ends_with(":") {
                    match get_echo_turn_off(raw_fd) {
                        Ok(true) => {
                            log::debug!("get_echo_turn_off ok");
                            if let Some(password) = initial_password.clone() {
                                let v = format!("{}\n", password);
                                if let Err(e) = file.write_all(v.as_bytes()) {
                                    let e = format!("Failed to send password: {e:?}");
                                    if let Err(e) = tx_to_ui.send(Message::ErrorDialog(e)) {
                                        log::error!("Channel error: {e:?}");
                                    }
                                    kill_child(child);
                                    break;
                                }
                                if is_su && !su_password_sent {
                                    su_password_sent = true;
                                    continue;
                                }
                                initial_password = None;
                                continue;
                            }
                            // In fact, su mode can only input password once
                            let err_msg = if first { "" } else { "Sorry, try again." };
                            first = false;
                            if let Err(e) =
                                tx_to_ui.send(Message::PasswordPrompt((err_msg.to_string(), false)))
                            {
                                log::error!("Channel error: {e:?}");
                                kill_child(child);
                                break;
                            }
                            match rx_from_ui.recv() {
                                Ok(Message::Password((_, password))) => {
                                    let v = format!("{}\n", password);
                                    if let Err(e) = file.write_all(v.as_bytes()) {
                                        let e = format!("Failed to send password: {e:?}");
                                        if let Err(e) = tx_to_ui.send(Message::ErrorDialog(e)) {
                                            log::error!("Channel error: {e:?}");
                                        }
                                        kill_child(child);
                                        break;
                                    }
                                }
                                Ok(Message::Cancel) => {
                                    log::info!("User canceled");
                                    kill_child(child);
                                    break;
                                }
                                _ => {
                                    log::error!("Unexpected message");
                                    break;
                                }
                            }
                        }
                        Ok(false) => log::warn!("get_echo_turn_off timeout"),
                        Err(e) => log::error!("get_echo_turn_off error: {:?}", e),
                    }
                } else {
                    saved_output = buf.clone();
                    if !last_line.is_empty() && initial_password.is_some() {
                        log::error!("received not empty line: {last_line}, clear initial password");
                        initial_password = None;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                // Child process is dead
                log::debug!("Read error: {:?}", e);
                break;
            }
        }
    }

    // Wait for child process
    let status = waitpid(child, None);
    log::info!("waitpid status: {:?}", status);
    let mut code = EXIT_CODE;
    match status {
        Ok(s) => match s {
            nix::sys::wait::WaitStatus::Exited(_pid, status) => {
                code = status;
            }
            _ => {}
        },
        Err(_) => {}
    }

    if code != 0 && !saved_output.is_empty() {
        if let Err(e) = tx_to_ui.send(Message::ErrorDialog(saved_output.clone())) {
            log::error!("Channel error: {e:?}");
            std::process::exit(code);
        }
        return Ok(());
    }
    if let Err(e) = tx_to_ui.send(Message::Exit(code)) {
        log::error!("Channel error: {e:?}");
        std::process::exit(code);
    }
    Ok(())
}

fn child(su_user: Option<String>, args: Vec<String>) -> ResultType<()> {
    // https://doc.rust-lang.org/std/env/consts/constant.OS.html
    let os = std::env::consts::OS;
    let bsd = os == "freebsd" || os == "dragonfly" || os == "netbsd" || os == "openbad";
    let mut params = vec!["sudo".to_string()];
    if su_user.is_some() {
        params.push("-S".to_string());
    }
    params.push("/bin/sh".to_string());
    params.push("-c".to_string());

    let command = args
        .iter()
        .map(|s| {
            if su_user.is_some() {
                s.to_string()
            } else {
                quote_shell_arg(s, true)
            }
        })
        .collect::<Vec<String>>()
        .join(" ");
    let mut command = if bsd {
        let lc = match std::env::var("LC_ALL") {
            Ok(lc_all) => {
                if lc_all.contains('\'') {
                    eprintln!(
                        "sudo: Detected attempt to inject privileged command via LC_ALL env({lc_all}). Exiting!\n",
                    );
                    std::process::exit(EXIT_CODE);
                }
                format!("LC_ALL='{lc_all}' ")
            }
            Err(_) => {
                format!("unset LC_ALL;")
            }
        };
        format!("{}exec {}", lc, command)
    } else {
        command.to_string()
    };
    if su_user.is_some() {
        command = format!("'{}'", quote_shell_arg(&command, false));
    }
    params.push(command);
    std::env::set_var("LC_ALL", "C");

    if let Some(user) = &su_user {
        let su_subcommand = params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(" ");
        params = vec![
            "su".to_string(),
            "-".to_string(),
            user.to_string(),
            "-c".to_string(),
            su_subcommand,
        ];
    }

    // allow failure here
    let _ = setsid();
    let mut cparams = vec![];
    for param in &params {
        cparams.push(CString::new(param.as_str())?);
    }
    let su_or_sudo = if su_user.is_some() { "su" } else { "sudo" };
    let res = execvp(CString::new(su_or_sudo)?.as_c_str(), &cparams);
    eprintln!("sudo: execvp error: {:?}", res);
    std::process::exit(EXIT_CODE);
}

fn get_echo_turn_off(fd: RawFd) -> Result<bool, Error> {
    let tios = termios::Termios::from_fd(fd)?;
    for _ in 0..10 {
        if tios.c_lflag & termios::ECHO == 0 {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(false)
}

fn turn_off_echo(fd: RawFd) -> Result<(), Error> {
    use termios::*;
    let mut termios = Termios::from_fd(fd)?;
    // termios.c_lflag &= !(ECHO | ECHONL | ICANON | IEXTEN);
    termios.c_lflag &= !ECHO;
    tcsetattr(fd, TCSANOW, &termios)?;
    Ok(())
}

pub extern "C" fn turn_on_echo_shutdown_hook() {
    let fd = std::io::stdin().as_raw_fd();
    if let Ok(mut termios) = termios::Termios::from_fd(fd) {
        termios.c_lflag |= termios::ECHO;
        termios::tcsetattr(fd, termios::TCSANOW, &termios).ok();
    }
}

fn kill_child(child: Pid) {
    unsafe { kill(child.as_raw(), Signal::SIGINT as _) };
    let mut res = 0;

    for _ in 0..10 {
        match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
            Ok(_) => {
                res = 1;
                break;
            }
            Err(_) => (),
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if res == 0 {
        log::info!("Force killing child process");
        unsafe { kill(child.as_raw(), Signal::SIGKILL as _) };
    }
}

fn password_prompt(
    username: &str,
    last_password: &str,
    err: &str,
    show_edit: bool,
) -> Option<(String, String)> {
    let dialog = gtk::Dialog::builder()
        .title(crate::get_app_name())
        .modal(true)
        .build();
    // https://docs.gtk.org/gtk4/method.Dialog.set_default_response.html
    dialog.set_default_response(gtk::ResponseType::Ok);
    let content_area = dialog.content_area();

    let label = gtk::Label::builder()
        .label(translate("Authentication Required".to_string()))
        .margin_top(10)
        .build();
    content_area.add(&label);

    let image = gtk::Image::from_icon_name(Some("avatar-default-symbolic"), gtk::IconSize::Dialog);
    image.set_margin_top(10);
    content_area.add(&image);

    let user_label = gtk::Label::new(Some(username));
    let edit_button = gtk::Button::new();
    edit_button.set_relief(gtk::ReliefStyle::None);
    let edit_icon =
        gtk::Image::from_icon_name(Some("document-edit-symbolic"), gtk::IconSize::Button.into());
    edit_button.set_image(Some(&edit_icon));
    edit_button.set_can_focus(false);
    let user_entry = gtk::Entry::new();
    user_entry.set_alignment(0.5);
    user_entry.set_width_request(100);
    let user_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    user_box.add(&user_label);
    user_box.add(&edit_button);
    user_box.add(&user_entry);
    user_box.set_halign(gtk::Align::Center);
    user_box.set_valign(gtk::Align::Center);
    user_box.set_vexpand(true);
    content_area.add(&user_box);

    edit_button.connect_clicked(
        glib::clone!(@weak user_label, @weak edit_button, @weak user_entry=>  move |_| {
            let username = user_label.text().to_string();
            user_entry.set_text(&username);
            user_label.hide();
            edit_button.hide();
            user_entry.show();
            user_entry.grab_focus();
        }),
    );

    let password_input = gtk::Entry::builder()
        .visibility(false)
        .input_purpose(gtk::InputPurpose::Password)
        .placeholder_text(translate("Password".to_string()))
        .margin_top(20)
        .margin_start(30)
        .margin_end(30)
        .activates_default(true)
        .text(last_password)
        .build();
    password_input.set_alignment(0.5);
    // https://docs.gtk.org/gtk3/signal.Entry.activate.html
    password_input.connect_activate(glib::clone!(@weak dialog => move |_| {
        dialog.response(gtk::ResponseType::Ok);
    }));
    content_area.add(&password_input);

    user_entry.connect_focus_out_event(
        glib::clone!(@weak user_label, @weak edit_button, @weak user_entry, @weak password_input => @default-return glib::Propagation::Proceed,  move |_, _| {
            let username = user_entry.text().to_string();
            user_label.set_text(&username);
            user_entry.hide();
            user_label.show();
            edit_button.show();
            glib::Propagation::Proceed
        }),
    );
    user_entry.connect_activate(
        glib::clone!(@weak user_label, @weak edit_button, @weak user_entry, @weak password_input => move |_| {
            let username = user_entry.text().to_string();
            user_label.set_text(&username);
            user_entry.hide();
            user_label.show();
            edit_button.show();
            password_input.grab_focus();
        }),
    );

    if !err.is_empty() {
        let err_label = gtk::Label::new(None);
        err_label.set_markup(&format!(
            "<span font='10' foreground='orange'>{}</span>",
            err
        ));
        err_label.set_selectable(true);
        content_area.add(&err_label);
    }

    let cancel_button = gtk::Button::builder()
        .label(translate("Cancel".to_string()))
        .hexpand(true)
        .build();
    cancel_button.connect_clicked(glib::clone!(@weak dialog => move |_| {
        dialog.response(gtk::ResponseType::Cancel);
    }));
    let authenticate_button = gtk::Button::builder()
        .label(translate("Authenticate".to_string()))
        .hexpand(true)
        .build();
    authenticate_button.connect_clicked(glib::clone!(@weak dialog => move |_| {
        dialog.response(gtk::ResponseType::Ok);
    }));
    let button_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .homogeneous(true)
        .spacing(10)
        .margin_top(10)
        .build();
    button_box.add(&cancel_button);
    button_box.add(&authenticate_button);
    content_area.add(&button_box);

    content_area.set_spacing(10);
    content_area.set_border_width(10);

    dialog.set_width_request(400);
    dialog.show_all();
    dialog.set_position(gtk::WindowPosition::Center);
    dialog.set_keep_above(true);
    password_input.grab_focus();
    user_entry.hide();
    if !show_edit {
        edit_button.hide();
    }
    dialog.check_resize();
    let response = dialog.run();
    dialog.hide();

    if response == gtk::ResponseType::Ok {
        let username = if user_entry.get_visible() {
            user_entry.text().to_string()
        } else {
            user_label.text().to_string()
        };
        Some((username, password_input.text().to_string()))
    } else {
        None
    }
}

fn error_dialog_and_exit(err_msg: &str, exit_code: i32) {
    log::error!("Error dialog: {err_msg}, exit code: {exit_code}");
    let dialog = gtk::MessageDialog::builder()
        .message_type(gtk::MessageType::Error)
        .title(crate::get_app_name())
        .text("Error")
        .secondary_text(err_msg)
        .modal(true)
        .buttons(gtk::ButtonsType::Ok)
        .build();
    dialog.set_position(gtk::WindowPosition::Center);
    dialog.set_keep_above(true);
    dialog.run();
    dialog.close();
    std::process::exit(exit_code);
}

fn quote_shell_arg(arg: &str, add_splash_if_match: bool) -> String {
    let mut rv = arg.to_string();
    let re = hbb_common::regex::Regex::new("(\\s|[][!\"#$&'()*,;<=>?\\^`{}|~])");
    let Ok(re) = re else {
        return rv;
    };
    if re.is_match(arg) {
        rv = rv.replace("'", "'\\''");
        if add_splash_if_match {
            rv = format!("'{}'", rv);
        }
    }
    rv
}
