use super::{linux::*, ResultType};
use hbb_common::{allow_err, bail, log, rand::prelude::*, tokio::time};
use pam;
use std::{
    collections::HashMap,
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use users::{get_user_by_name, os::unix::UserExt, User};

lazy_static::lazy_static! {
    static ref DESKTOP_RUNNING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref DESKTOP_INST: Arc<Mutex<Option<Desktop>>> = Arc::new(Mutex::new(None));
}

pub const VIRTUAL_X11_DESKTOP: &str = "xfce4";
pub const VIRTUAL_X11_DESKTOP_START: &str = "startxfce4";
pub const XFCE4_PANEL: &str = "xfce4-panel";
pub const GNOME_SESSION_BINARY: &str = "gnome-session-binary";
pub const ENV_DESKTOP_PROTOCAL: &str = "RUSTDESK_PROTOCAL";
pub const ENV_DESKTOP_PROTOCAL_WAYLAND: &str = "wayland";
pub const ENV_DESKTOP_PROTOCAL__X11: &str = "x11";
pub const ENV_DESKTOP_PROTOCAL_UNKNOWN: &str = "unknown";

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Protocal {
    Wayland,
    X11, // Xorg
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DesktopEnv {
    pub protocal: Protocal,
    pub username: String,
    pub uid: String,
    pub display: String,
    pub xauth: String,
}

#[derive(Debug)]
pub struct Desktop {
    env: DesktopEnv,
    child_exit: Arc<AtomicBool>,
    is_child_running: Arc<AtomicBool>,
}

fn check_update_env() {
    let mut inst = DESKTOP_INST.lock().unwrap();
    if let Some(inst) = &mut (*inst) {
        if !inst.is_child_running.load(Ordering::SeqCst) {
            inst.child_exit.store(true, Ordering::SeqCst);
            let old_env = inst.env.clone();
            allow_err!(inst.env.refresh());
            if !inst.env.is_same_env(&old_env) {
                inst.env.update_env();
                log::debug!("desktop env changed, {:?}", &inst.env);
            }
        }
    }
}

pub fn start_xdesktop() {
    std::thread::spawn(|| {
        if wait_xdesktop(20) {
            log::info!("Wait desktop: default");
        } else {
            log::info!("Wait desktop: none");
        }
        *DESKTOP_INST.lock().unwrap() = Some(Desktop::new());

        let interval = time::Duration::from_millis(super::SERVICE_INTERVAL);
        DESKTOP_RUNNING.store(true, Ordering::SeqCst);
        while DESKTOP_RUNNING.load(Ordering::SeqCst) {
            check_update_env();
            std::thread::sleep(interval);
        }
        log::info!("xdesktop update thread exit");
    });
}

pub fn stop_xdesktop() {
    DESKTOP_RUNNING.store(false, Ordering::SeqCst);
}

pub fn get_desktop_env() -> Option<DesktopEnv> {
    match &*DESKTOP_INST.lock().unwrap() {
        Some(inst) => Some(inst.env.clone()),
        None => None,
    }
}

pub fn try_start_x_session(username: &str, password: &str) -> ResultType<DesktopEnv> {
    let mut inst = DESKTOP_INST.lock().unwrap();
    if let Some(inst) = &mut (*inst) {
        let _ = inst.try_start_x_session(username, password)?;
        log::debug!("try_start_x_session, username: {}, {:?}", &username, &inst);
        Ok(inst.env.clone())
    } else {
        bail!(crate::server::LOGIN_MSG_XDESKTOP_NOT_INITED);
    }
}

fn wait_xdesktop(timeout_secs: u64) -> bool {
    let wait_begin = Instant::now();
    while wait_begin.elapsed().as_secs() < timeout_secs {
        let seat0 = get_values_of_seat0(&[0]);
        if !seat0[0].is_empty() {
            return true;
        }

        if let Ok(output) = run_cmds(format!(
            "ps -ef | grep -v 'grep' | grep -E 'gnome-session-binary|{}'",
            XFCE4_PANEL
        )) {
            if !output.is_empty() {
                log::info!("wait xdesktop: find xclient {}", &output);
                return true;
            }
        }

        std::thread::sleep(Duration::from_millis(super::SERVICE_INTERVAL));
    }

    false
}

impl DesktopEnv {
    pub fn new() -> Self {
        let xauth = get_env_var("XAUTHORITY");

        Self {
            protocal: Protocal::Unknown,
            username: "".to_owned(),
            uid: "".to_owned(),
            display: "".to_owned(),
            xauth: if xauth.is_empty() {
                "/tmp/.Xauthority".to_owned()
            } else {
                xauth
            },
        }
    }

    fn update_env(&self) {
        if self.is_ready() {
            std::env::set_var("DISPLAY", &self.display);
            std::env::set_var("XAUTHORITY", &self.xauth);
            std::env::set_var(ENV_DESKTOP_PROTOCAL, &self.protocal.to_string());
        } else {
            std::env::set_var("DISPLAY", "");
            std::env::set_var("XAUTHORITY", "");
            std::env::set_var(ENV_DESKTOP_PROTOCAL, &Protocal::Unknown.to_string());
        }
    }

    pub fn is_same_env(&self, other: &Self) -> bool {
        self.protocal == other.protocal
            && self.uid == other.uid
            && self.display == other.display
            && self.xauth == other.xauth
    }

    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        self.protocal == Protocal::X11
    }

    // The logic mainly fron https://github.com/neutrinolabs/xrdp/blob/34fe9b60ebaea59e8814bbc3ca5383cabaa1b869/sesman/session.c#L334.
    fn get_avail_display() -> ResultType<u32> {
        let display_range = 0..51;
        for i in display_range.clone() {
            if Self::is_x_server_running(i) {
                continue;
            }
            return Ok(i);
        }
        bail!("No avaliable display found in range {:?}", display_range)
    }

    fn is_x_server_running(display: u32) -> bool {
        Path::new(&format!("/tmp/.X11-unix/X{}", display)).exists()
            || Path::new(&format!("/tmp/.X{}-lock", display)).exists()
    }

    fn get_display(&mut self) {
        self.display = get_env_tries("DISPLAY", &self.uid, GNOME_SESSION_BINARY, 10);
        if self.display.is_empty() {
            self.display = get_env_tries("DISPLAY", &self.uid, XFCE4_PANEL, 10);
        }
        if self.display.is_empty() {
            self.display = Self::get_display_by_user(&self.username);
        }
        if self.display.is_empty() {
            self.display = ":0".to_owned();
        }
        self.display = self
            .display
            .replace(&whoami::hostname(), "")
            .replace("localhost", "");
    }

    fn get_xauth(&mut self) {
        self.xauth = get_env_tries("XAUTHORITY", &self.uid, GNOME_SESSION_BINARY, 10);
        if self.xauth.is_empty() {
            get_env_tries("XAUTHORITY", &self.uid, XFCE4_PANEL, 10);
        }

        let gdm = format!("/run/user/{}/gdm/Xauthority", self.uid);
        if self.xauth.is_empty() {
            self.xauth = if std::path::Path::new(&gdm).exists() {
                gdm
            } else {
                let username = &self.username;
                if username == "root" {
                    format!("/{}/.Xauthority", username)
                } else {
                    let tmp = format!("/home/{}/.Xauthority", username);
                    if std::path::Path::new(&tmp).exists() {
                        tmp
                    } else {
                        format!("/var/lib/{}/.Xauthority", username)
                    }
                }
            };
        }
    }

    // fixme: reduce loginctl
    fn get_env_seat0(&mut self) -> ResultType<bool> {
        let output = Command::new("loginctl").output()?;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if !line.contains("gdm") && line.contains("seat0") {
                if let Some(sid) = line.split_whitespace().nth(0) {
                    if Self::is_active(sid)? {
                        if let Some(uid) = line.split_whitespace().nth(1) {
                            self.uid = uid.to_owned();
                        }
                        if let Some(u) = line.split_whitespace().nth(2) {
                            self.username = u.to_owned();
                        }

                        self.protocal = Protocal::Unknown;
                        let type_output = Command::new("loginctl")
                            .args(vec!["show-session", "-p", "Type", sid])
                            .output()?;
                        let type_stdout = String::from_utf8_lossy(&type_output.stdout);

                        if type_stdout.contains("x11") {
                            self.protocal = Protocal::X11;
                            break;
                        } else if type_stdout.contains("wayland") {
                            self.protocal = Protocal::Wayland;
                        }
                    }
                }
            }
        }
        Ok(self.is_ready())
    }

    // some case, there is no seat0 https://github.com/rustdesk/rustdesk/issues/73
    fn get_env_active(&mut self) -> ResultType<bool> {
        let output = Command::new("loginctl").output()?;

        // set active Xorg session
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.contains("sessions listed.") {
                continue;
            }
            if let Some(sid) = line.split_whitespace().nth(0) {
                if Self::is_active(sid)? {
                    if Self::get_display_server_of_session(sid) == ENV_DESKTOP_PROTOCAL__X11 {
                        if let Some(uid) = line.split_whitespace().nth(1) {
                            self.uid = uid.to_owned();
                        }
                        if let Some(u) = line.split_whitespace().nth(2) {
                            self.username = u.to_owned();
                        }

                        self.protocal = Protocal::X11;
                    }
                }
            }
        }
        // // set active xfce4 session
        // for line in String::from_utf8_lossy(&output.stdout).lines() {
        //     if let Some(sid) = line.split_whitespace().nth(0) {
        //         if Self::is_active(sid)? {
        //             let tty_output = Command::new("loginctl")
        //                 .args(vec!["show-session", "-p", "TTY", sid])
        //                 .output()?;
        //             let tty: String = String::from_utf8_lossy(&tty_output.stdout)
        //                 .replace("TTY=", "")
        //                 .trim_end()
        //                 .into();

        //             let xfce_panel_info =
        //                 run_cmds(format!("ps -e | grep \"{}.\\\\+{}\"", tty, XFCE4_PANEL))?;
        //             if xfce_panel_info.trim_end().to_string() != "" {
        //                 if let Some(uid) = line.split_whitespace().nth(1) {
        //                     self.uid = uid.to_owned();
        //                 }
        //                 if let Some(u) = line.split_whitespace().nth(2) {
        //                     self.username = u.to_owned();
        //                 }
        //             }
        //         }
        //     }
        // }
        Ok(self.is_ready())
    }

    // fixme: dup
    fn get_display_server_of_session(session: &str) -> String {
        if let Ok(output) = Command::new("loginctl")
            .args(vec!["show-session", "-p", "Type", session])
            .output()
        // Check session type of the session
        {
            let display_server = String::from_utf8_lossy(&output.stdout)
                .replace("Type=", "")
                .trim_end()
                .into();
            if display_server == "tty" {
                // If the type is tty...
                if let Ok(output) = Command::new("loginctl")
                    .args(vec!["show-session", "-p", "TTY", session])
                    .output()
                // Get the tty number
                {
                    let tty: String = String::from_utf8_lossy(&output.stdout)
                        .replace("TTY=", "")
                        .trim_end()
                        .into();
                    if let Ok(xorg_results) =
                        run_cmds(format!("ps -e | grep \"{}.\\\\+Xorg\"", tty))
                    // And check if Xorg is running on that tty
                    {
                        if xorg_results.trim_end().to_string() != "" {
                            // If it is, manually return "x11", otherwise return tty
                            ENV_DESKTOP_PROTOCAL__X11.to_owned()
                        } else {
                            display_server
                        }
                    } else {
                        // If any of these commands fail just fall back to the display server
                        display_server
                    }
                } else {
                    display_server
                }
            } else {
                // If the session is not a tty, then just return the type as usual
                display_server
            }
        } else {
            "".to_owned()
        }
    }

    // fixme: remove
    fn is_active(sid: &str) -> ResultType<bool> {
        let output = Command::new("loginctl")
            .args(vec!["show-session", "-p", "State", sid])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).contains("active"))
    }

    fn get_display_by_user(user: &str) -> String {
        // log::debug!("w {}", &user);
        if let Ok(output) = std::process::Command::new("w").arg(&user).output() {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let mut iter = line.split_whitespace();
                let b = iter.nth(2);
                if let Some(b) = b {
                    if b.starts_with(":") {
                        return b.to_owned();
                    }
                }
            }
        }
        // above not work for gdm user
        //log::debug!("ls -l /tmp/.X11-unix/");
        let mut last = "".to_owned();
        if let Ok(output) = std::process::Command::new("ls")
            .args(vec!["-l", "/tmp/.X11-unix/"])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let mut iter = line.split_whitespace();
                let user_field = iter.nth(2);
                if let Some(x) = iter.last() {
                    if x.starts_with("X") {
                        last = x.replace("X", ":").to_owned();
                        if user_field == Some(&user) {
                            return last;
                        }
                    }
                }
            }
        }
        last
    }

    fn add_xauth_cookie(
        file: &str,
        display: &str,
        uid: u32,
        gid: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<()> {
        let randstr = (0..16)
            .map(|_| format!("{:02x}", random::<u8>()))
            .collect::<String>();
        let output = Command::new("xauth")
            .uid(uid)
            .gid(gid)
            .envs(envs)
            .args(vec!["-q", "-f", file, "add", display, ".", &randstr])
            .output()?;
        // xauth run success, even the following error occurs.
        // Ok(Output { status: ExitStatus(unix_wait_status(0)), stdout: "", stderr: "xauth:  file .Xauthority does not exist\n" })
        let errmsg = String::from_utf8_lossy(&output.stderr).to_string();
        if !errmsg.is_empty() {
            if !errmsg.contains("does not exist") {
                bail!("Failed to launch xauth, {}", errmsg)
            }
        }
        Ok(())
    }

    fn wait_x_server_running(pid: u32, display_num: u32, max_wait_secs: u64) -> ResultType<()> {
        let wait_begin = Instant::now();
        loop {
            if run_cmds(format!("ls /proc/{}", pid))?.is_empty() {
                bail!("X server exit");
            }

            if Self::is_x_server_running(display_num) {
                return Ok(());
            }
            if wait_begin.elapsed().as_secs() > max_wait_secs {
                bail!("Failed to wait xserver after {} seconds", max_wait_secs);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    fn refresh(&mut self) -> ResultType<bool> {
        *self = Self::new();
        if self.get_env_seat0()? || self.get_env_active()? {
            self.get_display();
            self.get_xauth();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        self.stop_children();
    }
}

impl Desktop {
    fn fatal_exit() {
        std::process::exit(0);
    }

    pub fn new() -> Self {
        Self {
            env: DesktopEnv::new(),
            child_exit: Arc::new(AtomicBool::new(true)),
            is_child_running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn try_start_x_session(&mut self, username: &str, password: &str) -> ResultType<()> {
        match get_user_by_name(username) {
            Some(userinfo) => {
                let mut client = pam::Client::with_password(pam_get_service_name())?;
                client
                    .conversation_mut()
                    .set_credentials(username, password);
                match client.authenticate() {
                    Ok(_) => {
                        if self.env.is_ready() && self.env.username == username {
                            return Ok(());
                        }

                        self.env.username = username.to_string();
                        self.env.uid = userinfo.uid().to_string();
                        self.env.protocal = Protocal::Unknown;
                        match self.start_x_session(&userinfo, password) {
                            Ok(_) => {
                                log::info!("Succeeded to start x11, update env {:?}", &self.env);
                                self.env.update_env();
                                Ok(())
                            }
                            Err(e) => {
                                self.env = DesktopEnv::new();
                                self.env.update_env();
                                bail!("failed to start x session, {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        bail!("failed to check user pass for {}, {}", username, e);
                    }
                }
            }
            None => {
                bail!("failed to get userinfo of {}", username);
            }
        }
    }

    fn start_x_session(&mut self, userinfo: &User, password: &str) -> ResultType<()> {
        self.stop_children();

        let display_num = DesktopEnv::get_avail_display()?;
        // "xServer_ip:display_num.screen_num"
        self.env.display = format!(":{}", display_num);

        let uid = userinfo.uid();
        let gid = userinfo.primary_group_id();
        let envs = HashMap::from([
            ("SHELL", userinfo.shell().to_string_lossy().to_string()),
            ("PATH", "/sbin:/bin:/usr/bin:/usr/local/bin".to_owned()),
            ("USER", self.env.username.clone()),
            ("UID", userinfo.uid().to_string()),
            ("HOME", userinfo.home_dir().to_string_lossy().to_string()),
            (
                "XDG_RUNTIME_DIR",
                format!("/run/user/{}", userinfo.uid().to_string()),
            ),
            // ("DISPLAY", self.display.clone()),
            // ("XAUTHORITY", self.xauth.clone()),
            // (ENV_DESKTOP_PROTOCAL, XProtocal::X11.to_string()),
        ]);
        let env = self.env.clone();
        self.child_exit.store(false, Ordering::SeqCst);
        let is_child_running = self.is_child_running.clone();

        let (tx_res, rx_res) = sync_channel(1);
        let password = password.to_string();
        // start x11
        std::thread::spawn(move || {
            match Self::start_x_session_thread(
                tx_res.clone(),
                is_child_running,
                env,
                uid,
                gid,
                display_num,
                password,
                envs,
            ) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to start x session thread");
                    allow_err!(tx_res.send(format!("Failed to start x session thread, {}", e)));
                }
            }
        });

        // wait x11
        match rx_res.recv_timeout(Duration::from_millis(10_000)) {
            Ok(res) => {
                if res == "" {
                    self.env.protocal = Protocal::X11;
                    Ok(())
                } else {
                    bail!(res)
                }
            }
            Err(e) => {
                bail!("Failed to recv x11 result {}", e)
            }
        }
    }

    fn start_x_session_thread(
        tx_res: SyncSender<String>,
        is_child_running: Arc<AtomicBool>,
        env: DesktopEnv,
        uid: u32,
        gid: u32,
        display_num: u32,
        password: String,
        envs: HashMap<&str, String>,
    ) -> ResultType<()> {
        let mut client = pam::Client::with_password(pam_get_service_name())?;
        client
            .conversation_mut()
            .set_credentials(&env.username, &password);
        client.authenticate()?;

        client.set_item(pam::PamItemType::TTY, &env.display)?;
        client.open_session()?;

        // fixme: FreeBSD kernel needs to login here.
        // see: https://github.com/neutrinolabs/xrdp/blob/a64573b596b5fb07ca3a51590c5308d621f7214e/sesman/session.c#L556

        let (child_xorg, child_wm) = Self::start_x11(&env, uid, gid, display_num, &envs)?;
        is_child_running.store(true, Ordering::SeqCst);

        log::info!("Start xorg and wm done, notify and wait xtop x11");
        allow_err!(tx_res.send("".to_owned()));

        Self::wait_stop_x11(child_xorg, child_wm);
        log::info!("Wait x11 stop done");
        Ok(())
    }

    fn wait_xorg_exit(child_xorg: &mut Child) -> ResultType<String> {
        if let Ok(_) = child_xorg.kill() {
            for _ in 0..3 {
                match child_xorg.try_wait() {
                    Ok(Some(status)) => return Ok(format!("Xorg exit with {}", status)),
                    Ok(None) => {}
                    Err(e) => {
                        // fatal error
                        log::error!("Failed to wait xorg process, {}", e);
                        bail!("Failed to wait xorg process, {}", e)
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            log::error!("Failed to wait xorg process, not exit");
            bail!("Failed to wait xorg process, not exit")
        } else {
            Ok("Xorg is already exited".to_owned())
        }
    }

    fn start_x11(
        env: &DesktopEnv,
        uid: u32,
        gid: u32,
        display_num: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<(Child, Child)> {
        log::debug!("envs of user {}: {:?}", &env.username, &envs);

        DesktopEnv::add_xauth_cookie(&env.xauth, &env.display, uid, gid, &envs)?;

        // Start Xorg
        let mut child_xorg = Self::start_x_server(&env.xauth, &env.display, uid, gid, &envs)?;

        log::info!("xorg started, wait 10 secs to ensuer x server is running");

        let max_wait_secs = 10;
        // wait x server running
        if let Err(e) =
            DesktopEnv::wait_x_server_running(child_xorg.id(), display_num, max_wait_secs)
        {
            match Self::wait_xorg_exit(&mut child_xorg) {
                Ok(msg) => log::info!("{}", msg),
                Err(e) => {
                    log::error!("{}", e);
                    Self::fatal_exit();
                }
            }
            bail!(e)
        }

        log::info!(
            "xorg is running, start x window manager with DISPLAY: {}, XAUTHORITY: {}",
            &env.display,
            &env.xauth
        );

        std::env::set_var("DISPLAY", &env.display);
        std::env::set_var("XAUTHORITY", &env.xauth);
        // start window manager (startwm.sh)
        let child_wm = match Self::start_x_window_manager(uid, gid, &envs) {
            Ok(c) => c,
            Err(e) => {
                match Self::wait_xorg_exit(&mut child_xorg) {
                    Ok(msg) => log::info!("{}", msg),
                    Err(e) => {
                        log::error!("{}", e);
                        Self::fatal_exit();
                    }
                }
                bail!(e)
            }
        };
        log::info!("x window manager is started");

        Ok((child_xorg, child_wm))
    }

    fn try_wait_x11_child_exit(child_xorg: &mut Child, child_wm: &mut Child) -> bool {
        match child_xorg.try_wait() {
            Ok(Some(status)) => {
                println!(
                    "=============================MYDEBUG Xorg exit with {}",
                    status
                );
                log::info!("Xorg exit with {}", status);
                return true;
            }
            Ok(None) => {}
            Err(e) => log::error!("Failed to wait xorg process, {}", e),
        }

        match child_wm.try_wait() {
            Ok(Some(status)) => {
                println!(
                    "=============================MYDEBUG: wm exit with {}",
                    status
                );
                log::info!("wm exit with {}", status);
                return true;
            }
            Ok(None) => {}
            Err(e) => log::error!("Failed to wait xorg process, {}", e),
        }
        false
    }

    fn wait_x11_children_exit(child_xorg: &mut Child, child_wm: &mut Child) {
        log::debug!("Try kill child process xorg");
        if let Ok(_) = child_xorg.kill() {
            let mut exited = false;
            for _ in 0..2 {
                match child_xorg.try_wait() {
                    Ok(Some(status)) => {
                        println!(
                            "=============================MYDEBUG Xorg exit with {}",
                            status
                        );
                        log::info!("Xorg exit with {}", status);
                        exited = true;
                        break;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        println!(
                            "=============================MYDEBUG Failed to wait xorg process, {}",
                            &e
                        );
                        log::error!("Failed to wait xorg process, {}", e);
                        Self::fatal_exit();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            if !exited {
                println!(
                    "=============================MYDEBUG Failed to wait child xorg, after kill()"
                );
                log::error!("Failed to wait child xorg, after kill()");
                // try kill -9?
            }
        }
        log::debug!("Try kill child process wm");
        if let Ok(_) = child_wm.kill() {
            let mut exited = false;
            for _ in 0..2 {
                match child_wm.try_wait() {
                    Ok(Some(status)) => {
                        println!(
                            "=============================MYDEBUG wm exit with {}",
                            status
                        );
                        log::info!("wm exit with {}", status);
                        exited = true;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        println!(
                            "=============================MYDEBUG Failed to wait wm process, {}",
                            &e
                        );
                        log::error!("Failed to wait wm process, {}", e);
                        Self::fatal_exit();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            if !exited {
                println!(
                    "=============================MYDEBUG Failed to wait child xorg, after kill()"
                );
                log::error!("Failed to wait child xorg, after kill()");
                // try kill -9?
            }
        }
    }

    fn try_wait_stop_x11(child_xorg: &mut Child, child_wm: &mut Child) -> bool {
        let mut inst = DESKTOP_INST.lock().unwrap();
        let mut exited = true;
        if let Some(inst) = &mut (*inst) {
            if inst.child_exit.load(Ordering::SeqCst) {
                exited = true;
            } else {
                exited = Self::try_wait_x11_child_exit(child_xorg, child_wm);
            }
            if exited {
                println!("=============================MYDEBUG begin to wait x11 children exit");
                Self::wait_x11_children_exit(child_xorg, child_wm);
                inst.is_child_running.store(false, Ordering::SeqCst);
                inst.child_exit.store(true, Ordering::SeqCst);
            }
        }
        exited
    }

    fn wait_stop_x11(mut child_xorg: Child, mut child_wm: Child) {
        loop {
            if Self::try_wait_stop_x11(&mut child_xorg, &mut child_wm) {
                break;
            }
            std::thread::sleep(Duration::from_millis(super::SERVICE_INTERVAL));
        }
    }

    fn get_xorg() -> ResultType<&'static str> {
        // Fedora 26 or later
        let xorg = "/usr/libexec/Xorg";
        if Path::new(xorg).is_file() {
            return Ok(xorg);
        }
        // Debian 9 or later
        let xorg = "/usr/lib/xorg/Xorg";
        if Path::new(xorg).is_file() {
            return Ok(xorg);
        }
        // Ubuntu 16.04 or later
        let xorg = "/usr/lib/xorg/Xorg";
        if Path::new(xorg).is_file() {
            return Ok(xorg);
        }
        // Arch Linux
        let xorg = "/usr/lib/xorg-server/Xorg";
        if Path::new(xorg).is_file() {
            return Ok(xorg);
        }
        // Arch Linux
        let xorg = "/usr/lib/Xorg";
        if Path::new(xorg).is_file() {
            return Ok(xorg);
        }
        // CentOS 7 /usr/bin/Xorg or param=Xorg

        log::warn!("Failed to find xorg, use default Xorg.\n Please add \"allowed_users=anybody\" to \"/etc/X11/Xwrapper.config\".");
        Ok("Xorg")
    }

    fn start_x_server(
        xauth: &str,
        display: &str,
        uid: u32,
        gid: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<Child> {
        let xorg = Self::get_xorg()?;
        log::info!("Use xorg: {}", &xorg);
        match Command::new(xorg)
            .envs(envs)
            .uid(uid)
            .gid(gid)
            .args(vec![
                "-noreset",
                "+extension",
                "GLX",
                "+extension",
                "RANDR",
                "+extension",
                "RENDER",
                //"-logfile",
                //"/tmp/RustDesk_xorg.log",
                "-config",
                "rustdesk/xorg.conf",
                "-auth",
                xauth,
                display,
            ])
            .spawn()
        {
            Ok(c) => Ok(c),
            Err(e) => {
                bail!("Failed to start Xorg with display {}, {}", display, e);
            }
        }
    }

    fn start_x_window_manager(
        uid: u32,
        gid: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<Child> {
        match Command::new("/etc/rustdesk/startwm.sh")
            .envs(envs)
            .uid(uid)
            .gid(gid)
            .spawn()
        {
            Ok(c) => Ok(c),
            Err(e) => {
                bail!("Failed to start window manager, {}", e);
            }
        }
    }

    fn stop_children(&mut self) {
        self.child_exit.store(true, Ordering::SeqCst);
        for _i in 1..10 {
            if !self.is_child_running.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(super::SERVICE_INTERVAL));
        }
        if self.is_child_running.load(Ordering::SeqCst) {
            log::warn!("xdesktop child is still running!");
        }
    }
}

fn pam_get_service_name() -> &'static str {
    if Path::new("/etc/pam.d/rustdesk").is_file() {
        "rustdesk"
    } else {
        "gdm"
    }
}

impl ToString for Protocal {
    fn to_string(&self) -> String {
        match self {
            Protocal::X11 => ENV_DESKTOP_PROTOCAL__X11.to_owned(),
            Protocal::Wayland => ENV_DESKTOP_PROTOCAL_WAYLAND.to_owned(),
            Protocal::Unknown => ENV_DESKTOP_PROTOCAL_UNKNOWN.to_owned(),
        }
    }
}
