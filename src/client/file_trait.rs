use hbb_common::{fs, log, message_proto::*};

use super::{Data, Interface};

pub trait FileManager: Interface {
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        feature = "cli",
        feature = "flutter"
    )))]
    fn get_home_dir(&self) -> String {
        fs::get_home_as_string()
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        feature = "cli",
        feature = "flutter"
    )))]
    fn get_next_job_id(&self) -> i32 {
        fs::get_next_job_id()
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        feature = "cli",
        feature = "flutter"
    )))]
    fn update_next_job_id(&self, id: i32) {
        fs::update_next_job_id(id);
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        feature = "cli",
        feature = "flutter"
    )))]
    fn read_dir(&self, path: String, include_hidden: bool) -> sciter::Value {
        match fs::read_dir(&fs::get_path(&path), include_hidden) {
            Err(_) => sciter::Value::null(),
            Ok(fd) => {
                use crate::ui::remote::make_fd;
                let mut m = make_fd(0, &fd.entries.to_vec(), false);
                m.set_item("path", path);
                m
            }
        }
    }

    fn cancel_job(&self, id: i32) {
        self.send(Data::CancelJob(id));
    }

    fn read_empty_dirs(&self, path: String, include_hidden: bool) {
        let mut msg_out = Message::new();
        let mut file_action = FileAction::new();
        file_action.set_read_empty_dirs(ReadEmptyDirs {
            path,
            include_hidden,
            ..Default::default()
        });
        msg_out.set_file_action(file_action);
        self.send(Data::Message(msg_out));
    }

    fn read_remote_dir(&self, path: String, include_hidden: bool) {
        let mut msg_out = Message::new();
        let mut file_action = FileAction::new();
        file_action.set_read_dir(ReadDir {
            path,
            include_hidden,
            ..Default::default()
        });
        msg_out.set_file_action(file_action);
        self.send(Data::Message(msg_out));
    }

    fn remove_file(&self, id: i32, path: String, file_num: i32, is_remote: bool) {
        self.send(Data::RemoveFile((id, path, file_num, is_remote)));
    }

    fn remove_dir_all(&self, id: i32, path: String, is_remote: bool, include_hidden: bool) {
        self.send(Data::RemoveDirAll((id, path, is_remote, include_hidden)));
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        feature = "cli",
        feature = "flutter"
    )))]
    fn confirm_delete_files(&self, id: i32, file_num: i32) {
        self.send(Data::ConfirmDeleteFiles((id, file_num)));
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        feature = "cli",
        feature = "flutter"
    )))]
    fn set_no_confirm(&self, id: i32) {
        self.send(Data::SetNoConfirm(id));
    }

    fn remove_dir(&self, id: i32, path: String, is_remote: bool) {
        if is_remote {
            self.send(Data::RemoveDir((id, path)));
        } else {
            fs::remove_all_empty_dir(&fs::get_path(&path)).ok();
        }
    }

    fn create_dir(&self, id: i32, path: String, is_remote: bool) {
        self.send(Data::CreateDir((id, path, is_remote)));
    }

    fn send_files(
        &self,
        id: i32,
        r#type: i32,
        path: String,
        to: String,
        file_num: i32,
        include_hidden: bool,
        is_remote: bool,
    ) {
        self.send(Data::SendFiles((
            id,
            r#type.into(),
            path,
            to,
            file_num,
            include_hidden,
            is_remote,
        )));
    }

    fn add_job(
        &self,
        id: i32,
        r#type: i32,
        path: String,
        to: String,
        file_num: i32,
        include_hidden: bool,
        is_remote: bool,
    ) {
        self.send(Data::AddJob((
            id,
            r#type.into(),
            path,
            to,
            file_num,
            include_hidden,
            is_remote,
        )));
    }

    fn resume_job(&self, id: i32, is_remote: bool) {
        self.send(Data::ResumeJob((id, is_remote)));
    }

    fn set_confirm_override_file(
        &self,
        id: i32,
        file_num: i32,
        need_override: bool,
        remember: bool,
        is_upload: bool,
    ) {
        log::info!(
            "confirm file transfer, job: {}, need_override: {}",
            id,
            need_override
        );
        self.send(Data::SetConfirmOverrideFile((
            id,
            file_num,
            need_override,
            remember,
            is_upload,
        )));
    }

    fn rename_file(&self, act_id: i32, path: String, new_name: String, is_remote: bool) {
        self.send(Data::RenameFile((act_id, path, new_name, is_remote)));
    }
}
