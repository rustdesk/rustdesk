#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]
include!(concat!(env!("OUT_DIR"), "/ffmpeg_vram_ffi.rs"));

use crate::{
    common::DataFormat::*,
    vram::inner::{DecodeCalls, EncodeCalls, InnerDecodeContext, InnerEncodeContext},
};

pub fn encode_calls() -> EncodeCalls {
    EncodeCalls {
        new: ffmpeg_vram_new_encoder,
        encode: ffmpeg_vram_encode,
        destroy: ffmpeg_vram_destroy_encoder,
        test: ffmpeg_vram_test_encode,
        set_bitrate: ffmpeg_vram_set_bitrate,
        set_framerate: ffmpeg_vram_set_framerate,
    }
}

pub fn decode_calls() -> DecodeCalls {
    DecodeCalls {
        new: ffmpeg_vram_new_decoder,
        decode: ffmpeg_vram_decode,
        destroy: ffmpeg_vram_destroy_decoder,
        test: ffmpeg_vram_test_decode,
    }
}

pub fn possible_support_encoders() -> Vec<InnerEncodeContext> {
    let dataFormats = vec![H264, H265];
    let mut v = vec![];
    for dataFormat in dataFormats.iter() {
        v.push(InnerEncodeContext {
            format: dataFormat.clone(),
        });
    }
    v
}

pub fn possible_support_decoders() -> Vec<InnerDecodeContext> {
    let codecs = vec![H264, H265];
    let mut v = vec![];
    for codec in codecs.iter() {
        v.push(InnerDecodeContext {
            data_format: codec.clone(),
        });
    }
    v
}
