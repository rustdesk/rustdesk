use crate::clipboard_file::clip_2_msg;
use clipboard::{
    platform::unix::{
        FileDescription, FileType, FILECONTENTS_FORMAT_NAME, FILEDESCRIPTORW_FORMAT_NAME,
    },
    ClipboardFile,
};
use hbb_common::{log, message_proto::Message};
use std::{
    collections::{HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const DOWNLOAD_CHUNK_SIZE: u64 = 4 * 1024 * 1024;
const MAX_CLIPBOARD_ENTRIES: usize = 16_384;
const MAX_CLIPBOARD_TOP_LEVEL_ENTRIES: usize = 64;
const MAX_CLIPBOARD_DESCRIPTOR_BYTES: usize = 4 + 592 * MAX_CLIPBOARD_ENTRIES;
const MAX_CLIPBOARD_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_CLIPBOARD_PATH_DEPTH: usize = 32;
const RETAINED_GENERATIONS: usize = 2;

pub struct OhosClipboardMaterializer {
    generation: u64,
    initialized_root: Option<PathBuf>,
    part_root: Option<PathBuf>,
    final_root: Option<PathBuf>,
    files: Vec<FileDescription>,
    top_level_names: Vec<PathBuf>,
    next_index: usize,
    current_index: Option<usize>,
    current_offset: u64,
    current_requested: u64,
    current_stream_id: i32,
    current_file: Option<File>,
    completed_roots: VecDeque<PathBuf>,
}

impl Default for OhosClipboardMaterializer {
    fn default() -> Self {
        Self {
            generation: 0,
            initialized_root: None,
            part_root: None,
            final_root: None,
            files: Vec::new(),
            top_level_names: Vec::new(),
            next_index: 0,
            current_index: None,
            current_offset: 0,
            current_requested: 0,
            current_stream_id: 0,
            current_file: None,
            completed_roots: VecDeque::new(),
        }
    }
}

impl OhosClipboardMaterializer {
    pub fn handle(
        &mut self,
        clip: ClipboardFile,
        conn_id: i32,
        configured_root: Option<&Path>,
    ) -> (Vec<Message>, Option<Vec<String>>) {
        match clip {
            ClipboardFile::FormatList { format_list } => {
                self.cancel();
                let Some(root) = configured_root else {
                    log::warn!("OHOS file clipboard root is not configured");
                    return (Vec::new(), None);
                };
                let has_contents = format_list
                    .iter()
                    .any(|(_, name)| name == FILECONTENTS_FORMAT_NAME);
                let descriptor_id = format_list
                    .iter()
                    .find(|(_, name)| name == FILEDESCRIPTORW_FORMAT_NAME)
                    .map(|(id, _)| *id);
                if !has_contents || descriptor_id.is_none() {
                    return (Vec::new(), None);
                }
                if let Err(error) = self.prepare_base_root(root) {
                    log::error!("Failed to prepare OHOS clipboard root: {error}");
                    return (Vec::new(), None);
                }
                let request = ClipboardFile::FormatDataRequest {
                    requested_format_id: descriptor_id.unwrap_or_default(),
                };
                (vec![clip_2_msg(request)], None)
            }
            ClipboardFile::FormatDataResponse {
                msg_flags,
                format_data,
            } => {
                if msg_flags != 0x1 || format_data.len() > MAX_CLIPBOARD_DESCRIPTOR_BYTES {
                    self.cancel();
                    return (Vec::new(), None);
                }
                let Some(root) = self.initialized_root.clone() else {
                    return (Vec::new(), None);
                };
                match FileDescription::parse_file_descriptors(format_data, conn_id)
                    .map_err(|error| error.to_string())
                    .and_then(|files| self.begin_generation(&root, files))
                    .and_then(|()| self.next_request_or_complete())
                {
                    Ok(MaterializerProgress::Request(message)) => (vec![message], None),
                    Ok(MaterializerProgress::Complete(paths)) => (Vec::new(), Some(paths)),
                    Err(error) => {
                        log::error!("Failed to begin OHOS file clipboard download: {error}");
                        self.cancel();
                        (Vec::new(), None)
                    }
                }
            }
            ClipboardFile::FileContentsResponse {
                msg_flags,
                stream_id,
                requested_data,
            } => match self.handle_contents_response(msg_flags, stream_id, &requested_data) {
                Ok(MaterializerProgress::Request(message)) => (vec![message], None),
                Ok(MaterializerProgress::Complete(paths)) => (Vec::new(), Some(paths)),
                Err(error) => {
                    log::error!("Failed to materialize OHOS clipboard file: {error}");
                    self.cancel();
                    (Vec::new(), None)
                }
            },
            ClipboardFile::TryEmpty => {
                self.cancel();
                (Vec::new(), None)
            }
            _ => (Vec::new(), None),
        }
    }

    fn prepare_base_root(&mut self, root: &Path) -> Result<(), String> {
        if self.initialized_root.as_deref() != Some(root) {
            fs::create_dir_all(root).map_err(|error| error.to_string())?;
            for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
                let entry = entry.map_err(|error| error.to_string())?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("generation-")
                    && entry
                        .file_type()
                        .map_err(|error| error.to_string())?
                        .is_dir()
                {
                    fs::remove_dir_all(entry.path()).map_err(|error| error.to_string())?;
                }
            }
            self.initialized_root = Some(root.to_path_buf());
            self.completed_roots.clear();
        }
        Ok(())
    }

    fn begin_generation(
        &mut self,
        base_root: &Path,
        files: Vec<FileDescription>,
    ) -> Result<(), String> {
        validate_descriptions(&files)?;
        self.cancel();
        self.generation = self.generation.wrapping_add(1);
        let generation_name = format!("generation-{}-{}", hbb_common::get_time(), self.generation);
        let part_root = base_root.join(format!("{generation_name}.part"));
        let final_root = base_root.join(generation_name);
        fs::create_dir_all(&part_root).map_err(|error| error.to_string())?;
        self.part_root = Some(part_root.clone());
        self.final_root = Some(final_root.clone());

        let mut top_level_names = Vec::new();
        let mut seen_top_level = HashSet::new();
        for description in &files {
            let destination = part_root.join(&description.name);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            if description.kind == FileType::Directory {
                fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            }
            if let Some(first) = description.name.components().next() {
                let name = PathBuf::from(first.as_os_str());
                if seen_top_level.insert(name.clone()) {
                    if top_level_names.len() >= MAX_CLIPBOARD_TOP_LEVEL_ENTRIES {
                        return Err(format!(
                            "clipboard file list exceeds {MAX_CLIPBOARD_TOP_LEVEL_ENTRIES} top-level entries"
                        ));
                    }
                    top_level_names.push(name);
                }
            }
        }

        self.files = files;
        self.top_level_names = top_level_names;
        self.next_index = 0;
        self.current_index = None;
        self.current_offset = 0;
        self.current_requested = 0;
        self.current_file = None;
        Ok(())
    }

    fn next_request_or_complete(&mut self) -> Result<MaterializerProgress, String> {
        while self.next_index < self.files.len() {
            let index = self.next_index;
            self.next_index += 1;
            let description = &self.files[index];
            if description.kind == FileType::Directory {
                continue;
            }
            let part_root = self
                .part_root
                .as_ref()
                .ok_or_else(|| "clipboard download root is missing".to_owned())?;
            let destination = part_root.join(&description.name);
            if description.size == 0 {
                File::create(&destination).map_err(|error| error.to_string())?;
                continue;
            }
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&destination)
                .map_err(|error| error.to_string())?;
            self.current_index = Some(index);
            self.current_offset = 0;
            self.current_file = Some(file);
            return self.build_range_request();
        }
        self.finalize_generation()
    }

    fn build_range_request(&mut self) -> Result<MaterializerProgress, String> {
        let index = self
            .current_index
            .ok_or_else(|| "clipboard file index is missing".to_owned())?;
        let size = self.files[index].size;
        let remaining = size.saturating_sub(self.current_offset);
        if remaining == 0 {
            if let Some(file) = self.current_file.take() {
                file.sync_all().map_err(|error| error.to_string())?;
            }
            self.current_index = None;
            return self.next_request_or_complete();
        }
        self.current_stream_id = self.current_stream_id.wrapping_add(1).max(1);
        self.current_requested = remaining.min(DOWNLOAD_CHUNK_SIZE);
        let offset = self.current_offset;
        let request = ClipboardFile::FileContentsRequest {
            stream_id: self.current_stream_id,
            list_index: i32::try_from(index).map_err(|error| error.to_string())?,
            dw_flags: 0x2,
            n_position_low: offset as u32 as i32,
            n_position_high: (offset >> 32) as u32 as i32,
            cb_requested: i32::try_from(self.current_requested)
                .map_err(|error| error.to_string())?,
            have_clip_data_id: false,
            clip_data_id: 0,
        };
        Ok(MaterializerProgress::Request(clip_2_msg(request)))
    }

    fn handle_contents_response(
        &mut self,
        msg_flags: i32,
        stream_id: i32,
        data: &[u8],
    ) -> Result<MaterializerProgress, String> {
        if msg_flags != 0x1 {
            return Err(format!(
                "peer returned clipboard file failure flags={msg_flags}"
            ));
        }
        if self.current_index.is_none() || stream_id != self.current_stream_id {
            return Err(format!("unexpected clipboard stream id={stream_id}"));
        }
        if data.len() as u64 != self.current_requested {
            return Err(format!(
                "invalid clipboard file chunk length={}",
                data.len()
            ));
        }
        let file = self
            .current_file
            .as_mut()
            .ok_or_else(|| "clipboard destination file is missing".to_owned())?;
        file.write_all(data).map_err(|error| error.to_string())?;
        self.current_offset = self
            .current_offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| "clipboard file size overflow".to_owned())?;
        let index = self.current_index.unwrap_or_default();
        if self.current_offset > self.files[index].size {
            return Err("clipboard file exceeded the declared size".to_owned());
        }
        self.build_range_request()
    }

    fn finalize_generation(&mut self) -> Result<MaterializerProgress, String> {
        let part_root = self
            .part_root
            .take()
            .ok_or_else(|| "clipboard partial root is missing".to_owned())?;
        let final_root = self
            .final_root
            .take()
            .ok_or_else(|| "clipboard final root is missing".to_owned())?;
        fs::rename(&part_root, &final_root).map_err(|error| error.to_string())?;
        let paths = self
            .top_level_names
            .iter()
            .map(|name| final_root.join(name).to_string_lossy().to_string())
            .collect::<Vec<_>>();
        self.files.clear();
        self.top_level_names.clear();
        self.completed_roots.push_back(final_root);
        while self.completed_roots.len() > RETAINED_GENERATIONS {
            if let Some(stale) = self.completed_roots.pop_front() {
                if let Err(error) = fs::remove_dir_all(&stale) {
                    log::warn!("Failed to remove stale OHOS clipboard generation: {error}");
                }
            }
        }
        Ok(MaterializerProgress::Complete(paths))
    }

    pub(crate) fn cancel(&mut self) {
        self.current_file.take();
        if let Some(part_root) = self.part_root.take() {
            if let Err(error) = fs::remove_dir_all(&part_root) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to remove partial OHOS clipboard generation: {error}");
                }
            }
        }
        self.final_root.take();
        self.files.clear();
        self.top_level_names.clear();
        self.next_index = 0;
        self.current_index = None;
        self.current_offset = 0;
        self.current_requested = 0;
    }
}

impl Drop for OhosClipboardMaterializer {
    fn drop(&mut self) {
        self.cancel();
    }
}

enum MaterializerProgress {
    Request(Message),
    Complete(Vec<String>),
}

fn validate_descriptions(files: &[FileDescription]) -> Result<(), String> {
    if files.is_empty() {
        return Err("clipboard file list is empty".to_owned());
    }
    if files.len() > MAX_CLIPBOARD_ENTRIES {
        return Err(format!(
            "clipboard file list exceeds {MAX_CLIPBOARD_ENTRIES} entries"
        ));
    }
    let mut names = HashSet::new();
    let mut regular_files = HashSet::new();
    let mut total_size = 0u64;
    for description in files {
        if description.kind == FileType::Symlink {
            return Err("clipboard symlinks are not supported".to_owned());
        }
        if description.name.components().count() > MAX_CLIPBOARD_PATH_DEPTH {
            return Err("clipboard path exceeds the maximum depth".to_owned());
        }
        if !names.insert(description.name.clone()) {
            return Err("clipboard file list contains duplicate paths".to_owned());
        }
        if description.kind == FileType::File {
            total_size = total_size
                .checked_add(description.size)
                .ok_or_else(|| "clipboard total size overflow".to_owned())?;
            regular_files.insert(description.name.clone());
        }
    }
    if total_size > MAX_CLIPBOARD_TOTAL_BYTES {
        return Err("clipboard file list exceeds 4 GiB".to_owned());
    }
    for description in files {
        for ancestor in description.name.ancestors().skip(1) {
            if regular_files.contains(ancestor) {
                return Err("clipboard path is nested below a regular file".to_owned());
            }
        }
    }
    Ok(())
}
