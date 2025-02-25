mod item_data_provider;
mod paste_observer;
mod paste_task;
pub mod pasteboard_context;

pub fn should_handle_msg(msg: &crate::ClipboardFile) -> bool {
    matches!(
        msg,
        crate::ClipboardFile::FormatList { .. }
            | crate::ClipboardFile::FormatDataResponse { .. }
            | crate::ClipboardFile::FileContentsResponse { .. }
            | crate::ClipboardFile::TryEmpty
    )
}
