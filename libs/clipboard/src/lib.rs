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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum ClipbaordFile {
    ServerFormatList {
        conn_id: i32,
        format_list: Vec<(i32, String)>,
    },
    ServerFormatListResponse {
        conn_id: i32,
        msg_flags: i32,
    },
    ServerFormatDataRequest {
        conn_id: i32,
        requested_format_id: i32,
    },
    ServerFormatDataResponse {
        conn_id: i32,
        msg_flags: i32,
        format_data: Vec<u8>,
    },
    FileContentsRequest {
        conn_id: i32,
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
        conn_id: i32,
        msg_flags: i32,
        stream_id: i32,
        requested_data: Vec<u8>,
    },
}

#[derive(Default)]
struct ConnEnabled {
    conn_enabled: HashMap<i32, bool>,
}

lazy_static::lazy_static! {
    static ref MSG_CHANNEL_CLIENT: (UnboundedSender<(i32, ClipbaordFile)>, TokioMutex<UnboundedReceiver<(i32, ClipbaordFile)>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, TokioMutex::new(rx))
    };

    static ref CLIP_CONN_ENABLED: Mutex<ConnEnabled> = Mutex::new(ConnEnabled::default());
}

#[inline(always)]
pub fn get_rx_clip_client<'a>() -> &'a TokioMutex<UnboundedReceiver<(i32, ClipbaordFile)>> {
    &MSG_CHANNEL_CLIENT.1
}

pub fn set_conn_enabled(conn_id: i32, enabled: bool) {
    let mut lock = CLIP_CONN_ENABLED.lock().unwrap();
    if conn_id != 0 {
        let _ = lock.conn_enabled.insert(conn_id, enabled);
    }
}

pub fn empty_clipboard(context: &mut Box<CliprdrClientContext>, conn_id: i32) -> bool {
    unsafe { TRUE == cliprdr::empty_cliprdr(&mut (**context), conn_id as u32) }
}

pub fn server_clip_file(
    context: &mut Box<CliprdrClientContext>,
    s_conn_id: i32,
    msg: ClipbaordFile,
) -> u32 {
    match msg {
        ClipbaordFile::ServerFormatList {
            mut conn_id,
            format_list,
        } => {
            if s_conn_id != 0 {
                conn_id = s_conn_id as i32;
            }
            log::debug!("server_format_list called");
            let ret = server_format_list(context, conn_id, format_list);
            log::debug!("server_format_list called, return {}", ret);
            ret
        }
        ClipbaordFile::ServerFormatListResponse {
            mut conn_id,
            msg_flags,
        } => {
            if s_conn_id != 0 {
                conn_id = s_conn_id as i32;
            }
            log::debug!("format_list_response called");
            let ret = server_format_list_response(context, conn_id, msg_flags);
            log::debug!("server_format_list_response called, return {}", ret);
            ret
        }
        ClipbaordFile::ServerFormatDataRequest {
            mut conn_id,
            requested_format_id,
        } => {
            if s_conn_id != 0 {
                conn_id = s_conn_id as i32;
            }
            log::debug!("format_data_request called");
            let ret = server_format_data_request(context, conn_id, requested_format_id);
            log::debug!("server_format_data_request called, return {}", ret);
            ret
        }
        ClipbaordFile::ServerFormatDataResponse {
            mut conn_id,
            msg_flags,
            format_data,
        } => {
            if s_conn_id != 0 {
                conn_id = s_conn_id as i32;
            }
            log::debug!("format_data_response called");
            let ret = server_format_data_response(context, conn_id, msg_flags, format_data);
            log::debug!("server_format_data_response called, return {}", ret);
            ret
        }
        ClipbaordFile::FileContentsRequest {
            mut conn_id,
            stream_id,
            list_index,
            dw_flags,
            n_position_low,
            n_position_high,
            cb_requested,
            have_clip_data_id,
            clip_data_id,
        } => {
            if s_conn_id != 0 {
                conn_id = s_conn_id as i32;
            }
            log::debug!("file_contents_request called");
            let ret = server_file_contents_request(
                context,
                conn_id,
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
            mut conn_id,
            msg_flags,
            stream_id,
            requested_data,
        } => {
            if s_conn_id != 0 {
                conn_id = s_conn_id as i32;
            }
            log::debug!("file_contents_response called");
            let ret = server_file_contents_response(
                context,
                conn_id,
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
    conn_id: i32,
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
            connID: conn_id as UINT32,
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
    conn_id: i32,
    msg_flags: i32,
) -> u32 {
    unsafe {
        let format_list_response = CLIPRDR_FORMAT_LIST_RESPONSE {
            connID: conn_id as UINT32,
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
    conn_id: i32,
    requested_format_id: i32,
) -> u32 {
    unsafe {
        let format_data_request = CLIPRDR_FORMAT_DATA_REQUEST {
            connID: conn_id as UINT32,
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
    conn_id: i32,
    msg_flags: i32,
    mut format_data: Vec<u8>,
) -> u32 {
    unsafe {
        let format_data_response = CLIPRDR_FORMAT_DATA_RESPONSE {
            connID: conn_id as UINT32,
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
    conn_id: i32,
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
            connID: conn_id as UINT32,
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
    conn_id: i32,
    msg_flags: i32,
    stream_id: i32,
    mut requested_data: Vec<u8>,
) -> u32 {
    unsafe {
        let file_contents_response = CLIPRDR_FILE_CONTENTS_RESPONSE {
            connID: conn_id as UINT32,
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

extern "C" fn check_enabled(conn_id: UINT32) -> BOOL {
    let lock = CLIP_CONN_ENABLED.lock().unwrap();

    let mut connd_enabled = false;
    if conn_id != 0 {
        if let Some(true) = lock.conn_enabled.get(&(conn_id as i32)) {
            connd_enabled = true;
        }
    } else {
        connd_enabled = true;
    }

    return if connd_enabled { TRUE } else { FALSE };
}

extern "C" fn client_format_list(
    _context: *mut CliprdrClientContext,
    clip_format_list: *const CLIPRDR_FORMAT_LIST,
) -> UINT {
    log::debug!("client_format_list called");

    let conn_id;
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
        conn_id = (*clip_format_list).connID as i32;
    }
    let data = ClipbaordFile::ServerFormatList {
        conn_id,
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

    let conn_id;
    let msg_flags;
    unsafe {
        conn_id = (*format_list_response).connID as i32;
        msg_flags = (*format_list_response).msgFlags as i32;
    }
    let data = ClipbaordFile::ServerFormatListResponse { conn_id, msg_flags };
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, data)).unwrap();

    0
}

extern "C" fn client_format_data_request(
    _context: *mut CliprdrClientContext,
    format_data_request: *const CLIPRDR_FORMAT_DATA_REQUEST,
) -> UINT {
    log::debug!("client_format_data_request called");

    let conn_id;
    let requested_format_id;
    unsafe {
        conn_id = (*format_data_request).connID as i32;
        requested_format_id = (*format_data_request).requestedFormatId as i32;
    }
    let data = ClipbaordFile::ServerFormatDataRequest {
        conn_id,
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
    log::debug!("cconn_idlient_format_data_response called");

    let conn_id;
    let msg_flags;
    let format_data;
    unsafe {
        conn_id = (*format_data_response).connID as i32;
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
    let data = ClipbaordFile::ServerFormatDataResponse {
        conn_id,
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

    let conn_id;
    let stream_id;
    let list_index;
    let dw_flags;
    let n_position_low;
    let n_position_high;
    let cb_requested;
    let have_clip_data_id;
    let clip_data_id;
    unsafe {
        conn_id = (*file_contents_request).connID as i32;
        stream_id = (*file_contents_request).streamId as i32;
        list_index = (*file_contents_request).listIndex as i32;
        dw_flags = (*file_contents_request).dwFlags as i32;
        n_position_low = (*file_contents_request).nPositionLow as i32;
        n_position_high = (*file_contents_request).nPositionHigh as i32;
        cb_requested = (*file_contents_request).cbRequested as i32;
        have_clip_data_id = (*file_contents_request).haveClipDataId == TRUE;
        clip_data_id = (*file_contents_request).clipDataId as i32;
    }

    let data = ClipbaordFile::FileContentsRequest {
        conn_id,
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

    let conn_id;
    let msg_flags;
    let stream_id;
    let requested_data;
    unsafe {
        conn_id = (*file_contents_response).connID as i32;
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
    let data = ClipbaordFile::FileContentsResponse {
        conn_id,
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
