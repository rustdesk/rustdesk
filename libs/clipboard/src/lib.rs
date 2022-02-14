use cliprdr::*;
use hbb_common::{
    log,
    message_proto::cliprdr as msg_cliprdr,
    message_proto::*,
    tokio::sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex as TokioMutex,
    },
    ResultType,
};
use std::{
    boxed::Box,
    ffi::{CStr, CString},
};

pub mod cliprdr;

#[derive(Debug)]
pub struct ConnID {
    pub server_conn_id: u32,
    pub remote_conn_id: u32,
}

lazy_static::lazy_static! {
    static ref MSG_CHANNEL_CLIENT: (UnboundedSender<(ConnID, Message)>, TokioMutex<UnboundedReceiver<(ConnID, Message)>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, TokioMutex::new(rx))
    };
}

#[inline(always)]
pub fn get_rx_client_msg<'a>() -> &'a TokioMutex<UnboundedReceiver<(ConnID, Message)>> {
    &MSG_CHANNEL_CLIENT.1
}

pub fn server_msg(context: &mut Box<CliprdrClientContext>, conn_id: ConnID, msg: Cliprdr) -> u32 {
    match msg.union {
        Some(msg_cliprdr::Union::ready(_)) => {
            // proc ready
            0
        }
        Some(msg_cliprdr::Union::format_list(req)) => {
            log::debug!("server_format_list called");
            let ret = server_format_list(context, conn_id, req);
            log::debug!("server_format_list called, return {}", ret);
            ret
        }
        Some(msg_cliprdr::Union::format_list_response(req)) => {
            log::debug!("format_list_response called");
            let ret = server_format_list_response(context, conn_id, req);
            log::debug!("server_format_list_response called, return {}", ret);
            ret
        }
        Some(msg_cliprdr::Union::format_data_request(req)) => {
            log::debug!("format_data_request called");
            let ret = server_format_data_request(context, conn_id, req);
            log::debug!("server_format_data_request called, return {}", ret);
            ret
        }
        Some(msg_cliprdr::Union::format_data_response(req)) => {
            log::debug!("format_data_response called");
            let ret = server_format_data_response(context, conn_id, req);
            log::debug!("server_format_data_response called, return {}", ret);
            ret
        }
        Some(msg_cliprdr::Union::file_contents_request(req)) => {
            log::debug!("file_contents_request called");
            let ret = server_file_contents_request(context, conn_id, req);
            log::debug!("server_file_contents_request called, return {}", ret);
            ret
        }
        Some(msg_cliprdr::Union::file_contents_response(req)) => {
            log::debug!("file_contents_response called");
            let ret = server_file_contents_response(context, conn_id, req);
            log::debug!("server_file_contents_response called, return {}", ret);
            ret
        }
        None => {
            unreachable!()
        }
    }
}

fn server_format_list(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    data: CliprdrServerFormatList,
) -> u32 {
    // do not check msgFlags for now
    unsafe {
        let num_formats = data.formats.len() as UINT32;
        let mut formats = data
            .formats
            .into_iter()
            .map(|format| {
                if format.format.is_empty() {
                    CLIPRDR_FORMAT {
                        formatId: format.id as UINT32,
                        formatName: 0 as *mut _,
                    }
                } else {
                    let n = match CString::new(format.format) {
                        Ok(n) => n,
                        Err(_) => CString::new("").unwrap(),
                    };
                    CLIPRDR_FORMAT {
                        formatId: format.id as UINT32,
                        formatName: n.into_raw(),
                    }
                }
            })
            .collect::<Vec<CLIPRDR_FORMAT>>();

        let server_conn_id = if conn_id.server_conn_id != 0 {
            conn_id.server_conn_id as UINT32
        } else {
            data.server_conn_id as UINT32
        };
        let remote_conn_id = if conn_id.remote_conn_id != 0 {
            conn_id.remote_conn_id as UINT32
        } else {
            data.remote_conn_id as UINT32
        };

        let format_list = CLIPRDR_FORMAT_LIST {
            serverConnID: server_conn_id,
            remoteConnID: remote_conn_id,
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
fn server_format_list_response(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    data: CliprdrServerFormatListResponse,
) -> u32 {
    unsafe {
        let server_conn_id = if conn_id.server_conn_id != 0 {
            conn_id.server_conn_id as UINT32
        } else {
            data.server_conn_id as UINT32
        };
        let remote_conn_id = if conn_id.remote_conn_id != 0 {
            conn_id.remote_conn_id as UINT32
        } else {
            data.remote_conn_id as UINT32
        };

        let format_list_response = CLIPRDR_FORMAT_LIST_RESPONSE {
            serverConnID: server_conn_id,
            remoteConnID: remote_conn_id,
            msgType: 0 as UINT16,
            msgFlags: data.msg_flags as UINT16,
            dataLen: 0 as UINT32,
        };

        let ret =
            (**context).ServerFormatListResponse.unwrap()(&mut (**context), &format_list_response);

        ret as u32
    }
}
fn server_format_data_request(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    data: CliprdrServerFormatDataRequest,
) -> u32 {
    unsafe {
        let server_conn_id = if conn_id.server_conn_id != 0 {
            conn_id.server_conn_id as UINT32
        } else {
            data.server_conn_id as UINT32
        };
        let remote_conn_id = if conn_id.remote_conn_id != 0 {
            conn_id.remote_conn_id as UINT32
        } else {
            data.remote_conn_id as UINT32
        };

        let format_data_request = CLIPRDR_FORMAT_DATA_REQUEST {
            serverConnID: server_conn_id,
            remoteConnID: remote_conn_id,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            requestedFormatId: data.requested_format_id as UINT32,
        };
        let ret =
            ((**context).ServerFormatDataRequest.unwrap())(&mut (**context), &format_data_request);
        ret as u32
    }
}
fn server_format_data_response(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    mut data: CliprdrServerFormatDataResponse,
) -> u32 {
    unsafe {
        let server_conn_id = if conn_id.server_conn_id != 0 {
            conn_id.server_conn_id as UINT32
        } else {
            data.server_conn_id as UINT32
        };
        let remote_conn_id = if conn_id.remote_conn_id != 0 {
            conn_id.remote_conn_id as UINT32
        } else {
            data.remote_conn_id as UINT32
        };
        let format_data_response = CLIPRDR_FORMAT_DATA_RESPONSE {
            serverConnID: server_conn_id,
            remoteConnID: remote_conn_id,
            msgType: 0 as UINT16,
            msgFlags: data.msg_flags as UINT16,
            dataLen: data.format_data.len() as UINT32,
            requestedFormatData: data.format_data.as_mut_ptr(),
        };
        let ret = ((**context).ServerFormatDataResponse.unwrap())(
            &mut (**context),
            &format_data_response,
        );
        ret as u32
    }
}
fn server_file_contents_request(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    data: CliprdrFileContentsRequest,
) -> u32 {
    unsafe {
        let server_conn_id = if conn_id.server_conn_id != 0 {
            conn_id.server_conn_id as UINT32
        } else {
            data.server_conn_id as UINT32
        };
        let remote_conn_id = if conn_id.remote_conn_id != 0 {
            conn_id.remote_conn_id as UINT32
        } else {
            data.remote_conn_id as UINT32
        };
        let file_contents_request = CLIPRDR_FILE_CONTENTS_REQUEST {
            serverConnID: server_conn_id,
            remoteConnID: remote_conn_id,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            streamId: data.stream_id as UINT32,
            listIndex: data.list_index as UINT32,
            dwFlags: data.dw_flags as UINT32,
            nPositionLow: data.n_position_low as UINT32,
            nPositionHigh: data.n_position_high as UINT32,
            cbRequested: data.cb_requested as UINT32,
            haveClipDataId: if data.have_clip_data_id { TRUE } else { FALSE },
            clipDataId: data.clip_data_id as UINT32,
        };
        let ret = ((**context).ServerFileContentsRequest.unwrap())(
            &mut (**context),
            &file_contents_request,
        );
        ret as u32
    }
}
fn server_file_contents_response(
    context: &mut Box<CliprdrClientContext>,
    conn_id: ConnID,
    mut data: CliprdrFileContentsResponse,
) -> u32 {
    unsafe {
        let server_conn_id = if conn_id.server_conn_id != 0 {
            conn_id.server_conn_id as UINT32
        } else {
            data.server_conn_id as UINT32
        };
        let remote_conn_id = if conn_id.remote_conn_id != 0 {
            conn_id.remote_conn_id as UINT32
        } else {
            data.remote_conn_id as UINT32
        };
        let file_contents_response = CLIPRDR_FILE_CONTENTS_RESPONSE {
            serverConnID: server_conn_id,
            remoteConnID: remote_conn_id,
            msgType: 0 as UINT16,
            msgFlags: data.msg_flags as UINT16,
            dataLen: 4 + data.requested_data.len() as UINT32,
            streamId: data.stream_id as UINT32,
            cbRequested: data.requested_data.len() as UINT32,
            requestedData: data.requested_data.as_mut_ptr(),
        };
        let ret = ((**context).ServerFileContentsResponse.unwrap())(
            &mut (**context),
            &file_contents_response,
        );
        ret as u32
    }
}

pub fn create_cliprdr_context(enable_files: bool, enable_others: bool) -> ResultType<Box<CliprdrClientContext>> {
    Ok(CliprdrClientContext::create(
        enable_files,
        enable_others,
        Some(client_format_list),
        Some(client_format_list_response),
        Some(client_format_data_request),
        Some(client_format_data_response),
        Some(client_file_contents_request),
        Some(client_file_contents_response),
    )?)
}

extern "C" fn client_format_list(
    _context: *mut CliprdrClientContext,
    format_list: *const CLIPRDR_FORMAT_LIST,
) -> UINT {
    log::debug!("client_format_list called");

    let mut data = CliprdrServerFormatList::default();
    unsafe {
        let mut i = 0u32;
        while i < (*format_list).numFormats {
            let format_data = &(*(*format_list).formats.offset(i as isize));
            if format_data.formatName.is_null() {
                data.formats.push(CliprdrFormat {
                    id: format_data.formatId as i32,
                    format: "".to_owned(),
                    ..Default::default()
                });
            } else {
                let format_name = CStr::from_ptr(format_data.formatName).to_str();
                let format_name = match format_name {
                    Ok(n) => n.to_owned(),
                    Err(_) => {
                        log::warn!("failed to get format name");
                        "".to_owned()
                    }
                };
                data.formats.push(CliprdrFormat {
                    id: format_data.formatId as i32,
                    format: format_name,
                    ..Default::default()
                });
            }
            i += 1;
        }

        data.server_conn_id = (*format_list).serverConnID as i32;
        data.remote_conn_id = (*format_list).remoteConnID as i32;
    }
    let conn_id = ConnID {
        server_conn_id: data.server_conn_id as u32,
        remote_conn_id: data.remote_conn_id as u32,
    };

    let mut msg = Message::new();
    let mut cliprdr = Cliprdr::new();
    cliprdr.set_format_list(data);
    msg.set_cliprdr(cliprdr);

    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, msg)).unwrap();

    0
}

extern "C" fn client_format_list_response(
    _context: *mut CliprdrClientContext,
    format_list_response: *const CLIPRDR_FORMAT_LIST_RESPONSE,
) -> UINT {
    log::debug!("client_format_list_response called");

    let mut data = CliprdrServerFormatListResponse::default();
    unsafe {
        data.server_conn_id = (*format_list_response).serverConnID as i32;
        data.remote_conn_id = (*format_list_response).remoteConnID as i32;
        data.msg_flags = (*format_list_response).msgFlags as i32;
    }
    let conn_id = ConnID {
        server_conn_id: data.server_conn_id as u32,
        remote_conn_id: data.remote_conn_id as u32,
    };

    let mut msg = Message::new();
    let mut cliprdr = Cliprdr::new();
    cliprdr.set_format_list_response(data);
    msg.set_cliprdr(cliprdr);
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, msg)).unwrap();

    0
}

extern "C" fn client_format_data_request(
    _context: *mut CliprdrClientContext,
    format_data_request: *const CLIPRDR_FORMAT_DATA_REQUEST,
) -> UINT {
    log::debug!("client_format_data_request called");

    let mut data = CliprdrServerFormatDataRequest::default();
    unsafe {
        data.server_conn_id = (*format_data_request).serverConnID as i32;
        data.remote_conn_id = (*format_data_request).remoteConnID as i32;
        data.requested_format_id = (*format_data_request).requestedFormatId as i32;
    }
    let conn_id = ConnID {
        server_conn_id: data.server_conn_id as u32,
        remote_conn_id: data.remote_conn_id as u32,
    };

    let mut msg = Message::new();
    let mut cliprdr = Cliprdr::new();
    cliprdr.set_format_data_request(data);
    msg.set_cliprdr(cliprdr);
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, msg)).unwrap();

    0
}

extern "C" fn client_format_data_response(
    _context: *mut CliprdrClientContext,
    format_data_response: *const CLIPRDR_FORMAT_DATA_RESPONSE,
) -> UINT {
    log::debug!("client_format_data_response called");

    let mut data = CliprdrServerFormatDataResponse::default();
    unsafe {
        data.server_conn_id = (*format_data_response).serverConnID as i32;
        data.remote_conn_id = (*format_data_response).remoteConnID as i32;
        data.msg_flags = (*format_data_response).msgFlags as i32;
        data.format_data = std::slice::from_raw_parts(
            (*format_data_response).requestedFormatData,
            (*format_data_response).dataLen as usize,
        )
        .to_vec();
    }
    let conn_id = ConnID {
        server_conn_id: data.server_conn_id as u32,
        remote_conn_id: data.remote_conn_id as u32,
    };

    let mut msg = Message::new();
    let mut cliprdr = Cliprdr::new();
    cliprdr.set_format_data_response(data);
    msg.set_cliprdr(cliprdr);
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, msg)).unwrap();

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

    let mut data = CliprdrFileContentsRequest::default();
    unsafe {
        data.server_conn_id = (*file_contents_request).serverConnID as i32;
        data.remote_conn_id = (*file_contents_request).remoteConnID as i32;
        data.stream_id = (*file_contents_request).streamId as i32;
        data.list_index = (*file_contents_request).listIndex as i32;
        data.dw_flags = (*file_contents_request).dwFlags as i32;
        data.n_position_low = (*file_contents_request).nPositionLow as i32;
        data.n_position_high = (*file_contents_request).nPositionHigh as i32;
        data.cb_requested = (*file_contents_request).cbRequested as i32;
        data.have_clip_data_id = (*file_contents_request).haveClipDataId == TRUE;
        data.clip_data_id = (*file_contents_request).clipDataId as i32;
    }
    let conn_id = ConnID {
        server_conn_id: data.server_conn_id as u32,
        remote_conn_id: data.remote_conn_id as u32,
    };

    let mut msg = Message::new();
    let mut cliprdr = Cliprdr::new();
    cliprdr.set_file_contents_request(data);
    msg.set_cliprdr(cliprdr);
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, msg)).unwrap();

    0
}

extern "C" fn client_file_contents_response(
    _context: *mut CliprdrClientContext,
    file_contents_response: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
) -> UINT {
    log::debug!("client_file_contents_response called");

    let mut data = CliprdrFileContentsResponse::default();
    unsafe {
        data.server_conn_id = (*file_contents_response).serverConnID as i32;
        data.remote_conn_id = (*file_contents_response).remoteConnID as i32;
        data.msg_flags = (*file_contents_response).msgFlags as i32;
        data.stream_id = (*file_contents_response).streamId as i32;
        data.requested_data = std::slice::from_raw_parts(
            (*file_contents_response).requestedData,
            (*file_contents_response).cbRequested as usize,
        )
        .to_vec();
    }
    let conn_id = ConnID {
        server_conn_id: data.server_conn_id as u32,
        remote_conn_id: data.remote_conn_id as u32,
    };

    let mut msg = Message::new();
    let mut cliprdr = Cliprdr::new();
    cliprdr.set_file_contents_response(data);
    msg.set_cliprdr(cliprdr);
    // no need to handle result here
    MSG_CHANNEL_CLIENT.0.send((conn_id, msg)).unwrap();

    0
}

#[cfg(test)]
mod tests {
    // #[test]
    // fn test_cliprdr_run() {
    //     super::cliprdr_run();
    // }
}
