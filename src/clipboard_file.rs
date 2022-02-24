use clipboard::ClipbaordFile;
use hbb_common::message_proto::*;

pub fn clip_2_msg(clip: ClipbaordFile) -> Message {
    match clip {
        ClipbaordFile::ServerFormatList {
            conn_id,
            format_list,
        } => {
            let mut formats: Vec<CliprdrFormat> = Vec::new();
            for v in format_list.iter() {
                formats.push(CliprdrFormat {
                    conn_id: 0,
                    id: v.0,
                    format: v.1.clone(),
                    ..Default::default()
                });
            }
            Message {
                union: Some(message::Union::cliprdr(Cliprdr {
                    union: Some(cliprdr::Union::format_list(CliprdrServerFormatList {
                        conn_id,
                        formats,
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            }
        }
        ClipbaordFile::ServerFormatListResponse { conn_id, msg_flags } => Message {
            union: Some(message::Union::cliprdr(Cliprdr {
                union: Some(cliprdr::Union::format_list_response(
                    CliprdrServerFormatListResponse {
                        conn_id,
                        msg_flags,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipbaordFile::ServerFormatDataRequest {
            conn_id,
            requested_format_id,
        } => Message {
            union: Some(message::Union::cliprdr(Cliprdr {
                union: Some(cliprdr::Union::format_data_request(
                    CliprdrServerFormatDataRequest {
                        conn_id,
                        requested_format_id,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipbaordFile::ServerFormatDataResponse {
            conn_id,
            msg_flags,
            format_data,
        } => Message {
            union: Some(message::Union::cliprdr(Cliprdr {
                union: Some(cliprdr::Union::format_data_response(
                    CliprdrServerFormatDataResponse {
                        conn_id,
                        msg_flags,
                        format_data,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipbaordFile::FileContentsRequest {
            conn_id,
            stream_id,
            list_index,
            dw_flags,
            n_position_low,
            n_position_high,
            cb_requested,
            have_clip_data_id,
            clip_data_id,
        } => Message {
            union: Some(message::Union::cliprdr(Cliprdr {
                union: Some(cliprdr::Union::file_contents_request(
                    CliprdrFileContentsRequest {
                        conn_id,
                        stream_id,
                        list_index,
                        dw_flags,
                        n_position_low,
                        n_position_high,
                        cb_requested,
                        have_clip_data_id,
                        clip_data_id,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipbaordFile::FileContentsResponse {
            conn_id,
            msg_flags,
            stream_id,
            requested_data,
        } => Message {
            union: Some(message::Union::cliprdr(Cliprdr {
                union: Some(cliprdr::Union::file_contents_response(
                    CliprdrFileContentsResponse {
                        conn_id,
                        msg_flags,
                        stream_id,
                        requested_data,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
    }
}

pub fn msg_2_clip(msg: Cliprdr) -> Option<ClipbaordFile> {
    match msg.union {
        Some(cliprdr::Union::format_list(data)) => {
            let mut format_list: Vec<(i32, String)> = Vec::new();
            for v in data.formats.iter() {
                format_list.push((v.id, v.format.clone()));
            }
            Some(ClipbaordFile::ServerFormatList {
                conn_id: data.conn_id,
                format_list,
            })
        }
        Some(cliprdr::Union::format_list_response(data)) => {
            Some(ClipbaordFile::ServerFormatListResponse {
                conn_id: data.conn_id,
                msg_flags: data.msg_flags,
            })
        }
        Some(cliprdr::Union::format_data_request(data)) => {
            Some(ClipbaordFile::ServerFormatDataRequest {
                conn_id: data.conn_id,
                requested_format_id: data.requested_format_id,
            })
        }
        Some(cliprdr::Union::format_data_response(data)) => {
            Some(ClipbaordFile::ServerFormatDataResponse {
                conn_id: data.conn_id,
                msg_flags: data.msg_flags,
                format_data: data.format_data,
            })
        }
        Some(cliprdr::Union::file_contents_request(data)) => {
            Some(ClipbaordFile::FileContentsRequest {
                conn_id: data.conn_id,
                stream_id: data.stream_id,
                list_index: data.list_index,
                dw_flags: data.dw_flags,
                n_position_low: data.n_position_low,
                n_position_high: data.n_position_high,
                cb_requested: data.cb_requested,
                have_clip_data_id: data.have_clip_data_id,
                clip_data_id: data.clip_data_id,
            })
        }
        Some(cliprdr::Union::file_contents_response(data)) => {
            Some(ClipbaordFile::FileContentsResponse {
                conn_id: data.conn_id,
                msg_flags: data.msg_flags,
                stream_id: data.stream_id,
                requested_data: data.requested_data,
            })
        }
        _ => None,
    }
}
