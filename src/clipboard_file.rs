use clipboard::ClipboardFile;
use hbb_common::message_proto::*;

pub fn clip_2_msg(clip: ClipboardFile) -> Message {
    match clip {
        ClipboardFile::MonitorReady => Message {
            union: Some(message::Union::Cliprdr(Cliprdr {
                union: Some(cliprdr::Union::Ready(CliprdrMonitorReady {
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipboardFile::FormatList { format_list } => {
            let mut formats: Vec<CliprdrFormat> = Vec::new();
            for v in format_list.iter() {
                formats.push(CliprdrFormat {
                    id: v.0,
                    format: v.1.clone(),
                    ..Default::default()
                });
            }
            Message {
                union: Some(message::Union::Cliprdr(Cliprdr {
                    union: Some(cliprdr::Union::FormatList(CliprdrServerFormatList {
                        formats,
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            }
        }
        ClipboardFile::FormatListResponse { msg_flags } => Message {
            union: Some(message::Union::Cliprdr(Cliprdr {
                union: Some(cliprdr::Union::FormatListResponse(
                    CliprdrServerFormatListResponse {
                        msg_flags,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipboardFile::FormatDataRequest {
            requested_format_id,
        } => Message {
            union: Some(message::Union::Cliprdr(Cliprdr {
                union: Some(cliprdr::Union::FormatDataRequest(
                    CliprdrServerFormatDataRequest {
                        requested_format_id,
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipboardFile::FormatDataResponse {
            msg_flags,
            format_data,
        } => Message {
            union: Some(message::Union::Cliprdr(Cliprdr {
                union: Some(cliprdr::Union::FormatDataResponse(
                    CliprdrServerFormatDataResponse {
                        msg_flags,
                        format_data: format_data.into(),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
        ClipboardFile::FileContentsRequest {
            stream_id,
            list_index,
            dw_flags,
            n_position_low,
            n_position_high,
            cb_requested,
            have_clip_data_id,
            clip_data_id,
        } => Message {
            union: Some(message::Union::Cliprdr(Cliprdr {
                union: Some(cliprdr::Union::FileContentsRequest(
                    CliprdrFileContentsRequest {
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
        ClipboardFile::FileContentsResponse {
            msg_flags,
            stream_id,
            requested_data,
        } => Message {
            union: Some(message::Union::Cliprdr(Cliprdr {
                union: Some(cliprdr::Union::FileContentsResponse(
                    CliprdrFileContentsResponse {
                        msg_flags,
                        stream_id,
                        requested_data: requested_data.into(),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            })),
            ..Default::default()
        },
    }
}

pub fn msg_2_clip(msg: Cliprdr) -> Option<ClipboardFile> {
    match msg.union {
        Some(cliprdr::Union::Ready(_)) => Some(ClipboardFile::MonitorReady),
        Some(cliprdr::Union::FormatList(data)) => {
            let mut format_list: Vec<(i32, String)> = Vec::new();
            for v in data.formats.iter() {
                format_list.push((v.id, v.format.clone()));
            }
            Some(ClipboardFile::FormatList { format_list })
        }
        Some(cliprdr::Union::FormatListResponse(data)) => Some(ClipboardFile::FormatListResponse {
            msg_flags: data.msg_flags,
        }),
        Some(cliprdr::Union::FormatDataRequest(data)) => Some(ClipboardFile::FormatDataRequest {
            requested_format_id: data.requested_format_id,
        }),
        Some(cliprdr::Union::FormatDataResponse(data)) => Some(ClipboardFile::FormatDataResponse {
            msg_flags: data.msg_flags,
            format_data: data.format_data.into(),
        }),
        Some(cliprdr::Union::FileContentsRequest(data)) => {
            Some(ClipboardFile::FileContentsRequest {
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
        Some(cliprdr::Union::FileContentsResponse(data)) => {
            Some(ClipboardFile::FileContentsResponse {
                msg_flags: data.msg_flags,
                stream_id: data.stream_id,
                requested_data: data.requested_data.into(),
            })
        }
        _ => None,
    }
}
