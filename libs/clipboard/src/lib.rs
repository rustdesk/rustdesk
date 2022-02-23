use cliprdr::*;
use hbb_common::{
    log,
    tokio::sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex as TokioMutex,
    },
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    boxed::Box,
    collections::HashMap,
    ffi::{CStr, CString},
    sync::Mutex,
};

pub mod cliprdr;

#[derive(Debug)]
pub struct ConnID {
    pub server_conn_id: u32,
    pub remote_conn_id: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum ClipbaordFile {
    ServerFormatList {
        server_conn_id: i32,
        remote_conn_id: i32,
        format_list: Vec<(i32, String)>,
    },
    ServerFormatListResponse {
        server_conn_id: i32,
        remote_conn_id: i32,
        msg_flags: i32,
    },
    ServerFormatDataRequest {
        server_conn_id: i32,
        remote_conn_id: i32,
        requested_format_id: i32,
    },
    ServerFormatDataResponse {
        server_conn_id: i32,
        remote_conn_id: i32,
        msg_flags: i32,
        format_data: Vec<u8>,
    },
    FileContentsRequest {
        server_conn_id: i32,
        remote_conn_id: i32,
        stream_id: i32,
        list_index: i32,
        dw_flags: i32,
        n_position_low: i32,
        n_position_high: i32,
        cb_requested: i32,
        have_clip_data_id: bool,
        clip_data_id: i32,
    },
    FileContentsResponse {
        server_conn_id: i32,
        remote_conn_id: i32,
        msg_flags: i32,
        stream_id: i32,
        requested_data: Vec<u8>,
    },
}

#[derive(Default)]
struct ConnEnabled {
    server_conn_enabled: HashMap<i32, bool>,
    remote_conn_enabled: HashMap<i32, bool>,
}

lazy_static::lazy_static! {
    static ref MSG_CHANNEL_CLIENT: (UnboundedSender<(ConnID, ClipbaordFile)>, TokioMutex<UnboundedReceiver<(ConnID, ClipbaordFile)>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, TokioMutex::new(rx))
    };

    static ref CLIP_CONN_ENABLED: Mutex<ConnEnabled> = Mutex::new(ConnEnabled::default());
}

#[inline(always)]
pub fn get_rx_clip_client<'a>() -> &'a TokioMutex<UnboundedReceiver<(ConnID, ClipbaordFile)>> {
    &MSG_CHANNEL_CLIENT.1
}

pub fn set_conn_enabled(server_conn_id: i32, remote_conn_id: i32, enabled: bool) {
    let mut lock = CLIP_CONN_ENABLED.lock().unwrap();
    if server_conn_id != 0 {
        let _ = lock.server_conn_enabled.insert(server_conn_id, enabled);
    }
    if remote_conn_id != 0 {
        let _ = lock.remote_conn_enabled.insert(remote_conn_id, enabled);
    }
}

pub fn empty_clipboard(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
) -> bool {
    unsafe {
        TRUE == cliprdr::empty_cliprdr(
            &mut (**context),
            server_conn_id as u32,
            remote_conn_id as u32,
        )
    }
}

pub fn server_clip_file(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    msg: ClipbaordFile,
) -> u32 {
    match msg {
        ClipbaordFile::ServerFormatList {
            mut server_conn_id,
            mut remote_conn_id,
            format_list,
        } => {
            if conn_id.server_conn_id != 0 {
                server_conn_id = conn_id.server_conn_id as i32;
            }
            if conn_id.remote_conn_id != 0 {
                remote_conn_id = conn_id.remote_conn_id as i32;
            }
            log::debug!("server_format_list called");
            let ret = server_format_list(context, server_conn_id, remote_conn_id, format_list);
            log::debug!("server_format_list called, return {}", ret);
            ret
        }
        ClipbaordFile::ServerFormatListResponse {
            mut server_conn_id,
            mut remote_conn_id,
            msg_flags,
        } => {
            if conn_id.server_conn_id != 0 {
                server_conn_id = conn_id.server_conn_id as i32;
            }
            if conn_id.remote_conn_id != 0 {
                remote_conn_id = conn_id.remote_conn_id as i32;
            }
            log::debug!("format_list_response called");
            let ret =
                server_format_list_response(context, server_conn_id, remote_conn_id, msg_flags);
            log::debug!("server_format_list_response called, return {}", ret);
            ret
        }
        ClipbaordFile::ServerFormatDataRequest {
            mut server_conn_id,
            mut remote_conn_id,
            requested_format_id,
        } => {
            if conn_id.server_conn_id != 0 {
                server_conn_id = conn_id.server_conn_id as i32;
            }
            if conn_id.remote_conn_id != 0 {
                remote_conn_id = conn_id.remote_conn_id as i32;
            }
            log::debug!("format_data_request called");
            let ret = server_format_data_request(
                context,
                server_conn_id,
                remote_conn_id,
                requested_format_id,
            );
            log::debug!("server_format_data_request called, return {}", ret);
            ret
        }
        ClipbaordFile::ServerFormatDataResponse {
            mut server_conn_id,
            mut remote_conn_id,
            msg_flags,
            format_data,
        } => {
            if conn_id.server_conn_id != 0 {
                server_conn_id = conn_id.server_conn_id as i32;
            }
            if conn_id.remote_conn_id != 0 {
                remote_conn_id = conn_id.remote_conn_id as i32;
            }
            log::debug!("format_data_response called");
            let ret = server_format_data_response(
                context,
                server_conn_id,
                remote_conn_id,
                msg_flags,
                format_data,
            );
            log::debug!("server_format_data_response called, return {}", ret);
            ret
        }
        ClipbaordFile::FileContentsRequest {
            mut server_conn_id,
            mut remote_conn_id,
            stream_id,
            list_index,
            dw_flags,
            n_position_low,
            n_position_high,
            cb_requested,
            have_clip_data_id,
            clip_data_id,
        } => {
            if conn_id.server_conn_id != 0 {
                server_conn_id = conn_id.server_conn_id as i32;
            }
            if conn_id.remote_conn_id != 0 {
                remote_conn_id = conn_id.remote_conn_id as i32;
            }
            log::debug!("file_contents_request called");
            let ret = server_file_contents_request(
                context,
                server_conn_id,
                remote_conn_id,
                stream_id,
                list_index,
                dw_flags,
                n_position_low,
                n_position_high,
                cb_requested,
                have_clip_data_id,
                clip_data_id,
            );
            log::debug!("server_file_contents_request called, return {}", ret);
            ret
        }
        ClipbaordFile::FileContentsResponse {
            mut server_conn_id,
            mut remote_conn_id,
            msg_flags,
            stream_id,
            requested_data,
        } => {
            if conn_id.server_conn_id != 0 {
                server_conn_id = conn_id.server_conn_id as i32;
            }
            if conn_id.remote_conn_id != 0 {
                remote_conn_id = conn_id.remote_conn_id as i32;
            }
            log::debug!("file_contents_response called");
            let ret = server_file_contents_response(
                context,
                server_conn_id,
                remote_conn_id,
                msg_flags,
                stream_id,
                requested_data,
            );
            log::debug!("server_file_contents_response called, return {}", ret);
            ret
        }
    }
}

pub fn server_format_list(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
    format_list: Vec<(i32, String)>,
) -> u32 {
    unsafe {
        let num_formats = format_list.len() as UINT32;
        let mut formats = format_list
            .into_iter()
            .map(|format| {
                if format.1.is_empty() {
                    CLIPRDR_FORMAT {
                        formatId: format.0 as UINT32,
                        formatName: 0 as *mut _,
                    }
                } else {
                    let n = match CString::new(format.1) {
                        Ok(n) => n,
                        Err(_) => CString::new("").unwrap(),
                    };
                    CLIPRDR_FORMAT {
                        formatId: format.0 as UINT32,
                        formatName: n.into_raw(),
                    }
                }
            })
            .collect::<Vec<CLIPRDR_FORMAT>>();

        let format_list = CLIPRDR_FORMAT_LIST {
            serverConnID: server_conn_id as UINT32,
            remoteConnID: remote_conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            numFormats: num_formats,
            formats: formats.as_mut_ptr(),
        };

        let ret = ((**context).ServerFormatList.unwrap())(&mut (**context), &format_list);

        for f in formats {
            if !f.formatName.is_null() {
                // retake pointer to free memory
                let _ = CString::from_raw(f.formatName);
            }
        }

        ret as u32
    }
}
pub fn server_format_list_response(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
    msg_flags: i32,
) -> u32 {
    unsafe {
        let format_list_response = CLIPRDR_FORMAT_LIST_RESPONSE {
            serverConnID: server_conn_id as UINT32,
            remoteConnID: remote_conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: msg_flags as UINT16,
            dataLen: 0 as UINT32,
        };

        let ret =
            (**context).ServerFormatListResponse.unwrap()(&mut (**context), &format_list_response);

        ret as u32
    }
}
pub fn server_format_data_request(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
    requested_format_id: i32,
) -> u32 {
    unsafe {
        let format_data_request = CLIPRDR_FORMAT_DATA_REQUEST {
            serverConnID: server_conn_id as UINT32,
            remoteConnID: remote_conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            requestedFormatId: requested_format_id as UINT32,
        };
        let ret =
            ((**context).ServerFormatDataRequest.unwrap())(&mut (**context), &format_data_request);
        ret as u32
    }
}
pub fn server_format_data_response(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
    msg_flags: i32,
    mut format_data: Vec<u8>,
) -> u32 {
    unsafe {
        let format_data_response = CLIPRDR_FORMAT_DATA_RESPONSE {
            serverConnID: server_conn_id as UINT32,
            remoteConnID: remote_conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: msg_flags as UINT16,
            dataLen: format_data.len() as UINT32,
            requestedFormatData: format_data.as_mut_ptr(),
        };
        let ret = ((**context).ServerFormatDataResponse.unwrap())(
            &mut (**context),
            &format_data_response,
        );
        ret as u32
    }
}
pub fn server_file_contents_request(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
    stream_id: i32,
    list_index: i32,
    dw_flags: i32,
    n_position_low: i32,
    n_position_high: i32,
    cb_requested: i32,
    have_clip_data_id: bool,
    clip_data_id: i32,
) -> u32 {
    unsafe {
        let file_contents_request = CLIPRDR_FILE_CONTENTS_REQUEST {
            serverConnID: server_conn_id as UINT32,
            remoteConnID: remote_conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            streamId: stream_id as UINT32,
            listIndex: list_index as UINT32,
            dwFlags: dw_flags as UINT32,
            nPositionLow: n_position_low as UINT32,
            nPositionHigh: n_position_high as UINT32,
            cbRequested: cb_requested as UINT32,
            haveClipDataId: if have_clip_data_id { TRUE } else { FALSE },
            clipDataId: clip_data_id as UINT32,
        };
        let ret = ((**context).ServerFileContentsRequest.unwrap())(
            &mut (**context),
            &file_contents_request,
        );
        ret as u32
    }
}
pub fn server_file_contents_response(
    context: &mut Box<CliprdrClientContext>,
    server_conn_id: i32,
    remote_conn_id: i32,
    msg_flags: i32,
    stream_id: i32,
    mut requested_data: Vec<u8>,
) -> u32 {
    unsafe {
        let file_contents_response = CLIPRDR_FILE_CONTENTS_RESPONSE {
            serverConnID: server_conn_id as UINT32,
            remoteConnID: remote_conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: msg_flags as UINT16,
            dataLen: 4 + requested_data.len() as UINT32,
            streamId: stream_id as UINT32,
            cbRequested: requested_data.len() as UINT32,
            requestedData: requested_data.as_mut_ptr(),
        };
        let ret = ((**context).ServerFileContentsResponse.unwrap())(
            &mut (**context),
            &file_contents_response,
        );
        ret as u32
    }
}

pub fn create_cliprdr_context(
    enable_files: bool,
    enable_others: bool,
) -> ResultType<Box<CliprdrClientContext>> {
    Ok(CliprdrClientContext::create(
        enable_files,
        enable_others,
        Some(check_enabled),
        Some(client_format_list),
        Some(client_format_list_response),
        Some(client_format_data_request),
        Some(client_format_data_response),
        Some(client_file_contents_request),
        Some(client_file_contents_response),
    )?)
}

extern "C" fn check_enabled(server_conn_id: UINT32, remote_conn_id: UINT32) -> BOOL {
    let lock = CLIP_CONN_ENABLED.lock().unwrap();
    if server_conn_id == 0 && remote_conn_id == 0 {
        return FALSE;
    }

    let mut server_conn_enabled = false;
    if server_conn_id != 0 {
        if let Some(true) = lock.server_conn_enabled.get(&(server_conn_id as i32)) {
            server_conn_enabled = true;
        }
    } else {
        server_conn_enabled = true;
    }

    // let mut remote_conn_enabled = false;
    // remote connection is always enabled
    // if remote_conn_id != 0 {
    //     if let Some(true) = lock.remote_conn_enabled.get(&(remote_conn_id as i32)) {
    //         remote_conn_enabled = true;
    //     }
    // } else {
    //     remote_conn_enabled = true;
    // }
    let remote_conn_enabled = true;

    if server_conn_enabled && remote_conn_enabled {
        return TRUE;
    } else {
        return FALSE;
    }
}

extern "C" fn client_format_list(
    _context: *mut CliprdrClientContext,
    clip_format_list: *const CLIPRDR_FORMAT_LIST,
) -> UINT {
    log::debug!("client_format_list called");

    let server_conn_id;
    let remote_conn_id;
    let mut format_list: Vec<(i32, String)> = Vec::new();
    unsafe {
        let mut i = 0u32;
        while i < (*clip_format_list).numFormats {
            let format_data = &(*(*clip_format_list).formats.offset(i as isize));
            if format_data.formatName.is_null() {
                format_list.push((format_data.formatId as i32, "".to_owned()));
            } else {
                let format_name = CStr::from_ptr(format_data.formatName).to_str();
                let format_name = match format_name {
                    Ok(n) => n.to_owned(),
                    Err(_) => {
                        log::warn!("failed to get format name");
                        "".to_owned()
                    }
                };
                format_list.push((format_data.formatId as i32, format_name));
            }
            // log::debug!("format list item {}: format id: {}, format name: {}", i, format_data.formatId, &format_name);
            i += 1;
        }
        server_conn_id = (*clip_format_list).serverConnID as i32;
        remote_conn_id = (*clip_format_list).remoteConnID as i32;
    }
    let conn_id = ConnID {
        server_conn_id: server_conn_id as u32,
        remote_conn_id: remote_conn_id as u32,
    };
    let data = ClipbaordFile::ServerFormatList {
        server_conn_id,
        remote_conn_id,
        format_list,
    };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

extern "C" fn client_format_list_response(
    _context: *mut CliprdrClientContext,
    format_list_response: *const CLIPRDR_FORMAT_LIST_RESPONSE,
) -> UINT {
    log::debug!("client_format_list_response called");

    let server_conn_id;
    let remote_conn_id;
    let msg_flags;
    unsafe {
        server_conn_id = (*format_list_response).serverConnID as i32;
        remote_conn_id = (*format_list_response).remoteConnID as i32;
        msg_flags = (*format_list_response).msgFlags as i32;
    }
    let conn_id = ConnID {
        server_conn_id: server_conn_id as u32,
        remote_conn_id: remote_conn_id as u32,
    };
    let data = ClipbaordFile::ServerFormatListResponse {
        server_conn_id,
        remote_conn_id,
        msg_flags,
    };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

extern "C" fn client_format_data_request(
    _context: *mut CliprdrClientContext,
    format_data_request: *const CLIPRDR_FORMAT_DATA_REQUEST,
) -> UINT {
    log::debug!("client_format_data_request called");

    let server_conn_id;
    let remote_conn_id;
    let requested_format_id;
    unsafe {
        server_conn_id = (*format_data_request).serverConnID as i32;
        remote_conn_id = (*format_data_request).remoteConnID as i32;
        requested_format_id = (*format_data_request).requestedFormatId as i32;
    }
    let conn_id = ConnID {
        server_conn_id: server_conn_id as u32,
        remote_conn_id: remote_conn_id as u32,
    };
    let data = ClipbaordFile::ServerFormatDataRequest {
        server_conn_id,
        remote_conn_id,
        requested_format_id,
    };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

extern "C" fn client_format_data_response(
    _context: *mut CliprdrClientContext,
    format_data_response: *const CLIPRDR_FORMAT_DATA_RESPONSE,
) -> UINT {
    log::debug!("client_format_data_response called");

    let server_conn_id;
    let remote_conn_id;
    let msg_flags;
    let format_data;
    unsafe {
        server_conn_id = (*format_data_response).serverConnID as i32;
        remote_conn_id = (*format_data_response).remoteConnID as i32;
        msg_flags = (*format_data_response).msgFlags as i32;
        if (*format_data_response).requestedFormatData.is_null() {
            format_data = Vec::new();
        } else {
            format_data = std::slice::from_raw_parts(
                (*format_data_response).requestedFormatData,
                (*format_data_response).dataLen as usize,
            )
            .to_vec();
        }
    }
    let conn_id = ConnID {
        server_conn_id: server_conn_id as u32,
        remote_conn_id: remote_conn_id as u32,
    };
    let data = ClipbaordFile::ServerFormatDataResponse {
        server_conn_id,
        remote_conn_id,
        msg_flags,
        format_data,
    };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

extern "C" fn client_file_contents_request(
    _context: *mut CliprdrClientContext,
    file_contents_request: *const CLIPRDR_FILE_CONTENTS_REQUEST,
) -> UINT {
    log::debug!("client_file_contents_request called");

    // TODO: support huge file?
    // if (!cliprdr->hasHugeFileSupport)
    // {
    // 	if (((UINT64)fileContentsRequest->cbRequested + fileContentsRequest->nPositionLow) >
    // 	    UINT32_MAX)
    // 		return ERROR_INVALID_PARAMETER;
    // 	if (fileContentsRequest->nPositionHigh != 0)
    // 		return ERROR_INVALID_PARAMETER;
    // }

    let server_conn_id;
    let remote_conn_id;
    let stream_id;
    let list_index;
    let dw_flags;
    let n_position_low;
    let n_position_high;
    let cb_requested;
    let have_clip_data_id;
    let clip_data_id;
    unsafe {
        server_conn_id = (*file_contents_request).serverConnID as i32;
        remote_conn_id = (*file_contents_request).remoteConnID as i32;
        stream_id = (*file_contents_request).streamId as i32;
        list_index = (*file_contents_request).listIndex as i32;
        dw_flags = (*file_contents_request).dwFlags as i32;
        n_position_low = (*file_contents_request).nPositionLow as i32;
        n_position_high = (*file_contents_request).nPositionHigh as i32;
        cb_requested = (*file_contents_request).cbRequested as i32;
        have_clip_data_id = (*file_contents_request).haveClipDataId == TRUE;
        clip_data_id = (*file_contents_request).clipDataId as i32;
    }
    let conn_id = ConnID {
        server_conn_id: server_conn_id as u32,
        remote_conn_id: remote_conn_id as u32,
    };

    let data = ClipbaordFile::FileContentsRequest {
        server_conn_id,
        remote_conn_id,
        stream_id,
        list_index,
        dw_flags,
        n_position_low,
        n_position_high,
        cb_requested,
        have_clip_data_id,
        clip_data_id,
    };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

extern "C" fn client_file_contents_response(
    _context: *mut CliprdrClientContext,
    file_contents_response: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
) -> UINT {
    log::debug!("client_file_contents_response called");

    let server_conn_id;
    let remote_conn_id;
    let msg_flags;
    let stream_id;
    let requested_data;
    unsafe {
        server_conn_id = (*file_contents_response).serverConnID as i32;
        remote_conn_id = (*file_contents_response).remoteConnID as i32;
        msg_flags = (*file_contents_response).msgFlags as i32;
        stream_id = (*file_contents_response).streamId as i32;
        if (*file_contents_response).requestedData.is_null() {
            requested_data = Vec::new();
        } else {
            requested_data = std::slice::from_raw_parts(
                (*file_contents_response).requestedData,
                (*file_contents_response).cbRequested as usize,
            )
            .to_vec();
        }
    }
    let conn_id = ConnID {
        server_conn_id: server_conn_id as u32,
        remote_conn_id: remote_conn_id as u32,
    };

    let data = ClipbaordFile::FileContentsResponse {
        server_conn_id,
        remote_conn_id,
        msg_flags,
        stream_id,
        requested_data,
    };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

#[cfg(test)]
mod tests {
    // #[test]
    // fn test_cliprdr_run() {
    //     super::cliprdr_run();
    // }
}
