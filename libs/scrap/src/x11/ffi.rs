#![allow(non_camel_case_types)]

use hbb_common::libc::c_void;

#[link(name = "xcb")]
#[link(name = "xcb-shm")]
#[link(name = "xcb-randr")]
extern "C" {
    pub fn xcb_connect(displayname: *const i8, screenp: *mut i32) -> *mut xcb_connection_t;

    pub fn xcb_disconnect(c: *mut xcb_connection_t);

    pub fn xcb_connection_has_error(c: *mut xcb_connection_t) -> i32;

    pub fn xcb_get_setup(c: *mut xcb_connection_t) -> *const xcb_setup_t;

    pub fn xcb_setup_roots_iterator(r: *const xcb_setup_t) -> xcb_screen_iterator_t;

    pub fn xcb_screen_next(i: *mut xcb_screen_iterator_t);

    pub fn xcb_generate_id(c: *mut xcb_connection_t) -> u32;

    pub fn xcb_shm_attach(
        c: *mut xcb_connection_t,
        shmseg: xcb_shm_seg_t,
        shmid: u32,
        read_only: u8,
    ) -> xcb_void_cookie_t;

    pub fn xcb_shm_detach(c: *mut xcb_connection_t, shmseg: xcb_shm_seg_t) -> xcb_void_cookie_t;

    pub fn xcb_shm_get_image_unchecked(
        c: *mut xcb_connection_t,
        drawable: xcb_drawable_t,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
        format: u8,
        shmseg: xcb_shm_seg_t,
        offset: u32,
    ) -> xcb_shm_get_image_cookie_t;

    pub fn xcb_shm_get_image_reply(
        c: *mut xcb_connection_t,
        cookie: xcb_shm_get_image_cookie_t,
        e: *mut *mut xcb_generic_error_t,
    ) -> *mut xcb_shm_get_image_reply_t;

    pub fn xcb_randr_get_monitors_unchecked(
        c: *mut xcb_connection_t,
        window: xcb_window_t,
        get_active: u8,
    ) -> xcb_randr_get_monitors_cookie_t;

    pub fn xcb_randr_get_monitors_reply(
        c: *mut xcb_connection_t,
        cookie: xcb_randr_get_monitors_cookie_t,
        e: *mut *mut xcb_generic_error_t,
    ) -> *mut xcb_randr_get_monitors_reply_t;

    pub fn xcb_randr_get_monitors_monitors_iterator(
        r: *const xcb_randr_get_monitors_reply_t,
    ) -> xcb_randr_monitor_info_iterator_t;

    pub fn xcb_randr_monitor_info_next(i: *mut xcb_randr_monitor_info_iterator_t);

    pub fn xcb_get_atom_name(
        c: *mut xcb_connection_t,
        atom: xcb_atom_t,
    ) -> xcb_get_atom_name_cookie_t;

    pub fn xcb_get_atom_name_reply(
        c: *mut xcb_connection_t,
        cookie: xcb_get_atom_name_cookie_t,
        e: *mut *mut xcb_generic_error_t,
    ) -> *const xcb_get_atom_name_reply_t;

    pub fn xcb_get_atom_name_name(reply: *const xcb_get_atom_name_request_t) -> *const u8;

    pub fn xcb_get_atom_name_name_length(reply: *const xcb_get_atom_name_reply_t) -> i32;
}

pub const XCB_IMAGE_FORMAT_Z_PIXMAP: u8 = 2;

pub type xcb_atom_t = u32;
pub type xcb_connection_t = c_void;
pub type xcb_window_t = u32;
pub type xcb_keycode_t = u8;
pub type xcb_visualid_t = u32;
pub type xcb_timestamp_t = u32;
pub type xcb_colormap_t = u32;
pub type xcb_shm_seg_t = u32;
pub type xcb_drawable_t = u32;
pub type xcb_get_atom_name_cookie_t = u32;
pub type xcb_get_atom_name_reply_t = u32;
pub type xcb_get_atom_name_request_t = xcb_get_atom_name_reply_t;

#[repr(C)]
pub struct xcb_setup_t {
    pub status: u8,
    pub pad0: u8,
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub length: u16,
    pub release_number: u32,
    pub resource_id_base: u32,
    pub resource_id_mask: u32,
    pub motion_buffer_size: u32,
    pub vendor_len: u16,
    pub maximum_request_length: u16,
    pub roots_len: u8,
    pub pixmap_formats_len: u8,
    pub image_byte_order: u8,
    pub bitmap_format_bit_order: u8,
    pub bitmap_format_scanline_unit: u8,
    pub bitmap_format_scanline_pad: u8,
    pub min_keycode: xcb_keycode_t,
    pub max_keycode: xcb_keycode_t,
    pub pad1: [u8; 4],
}

#[repr(C)]
pub struct xcb_screen_iterator_t {
    pub data: *mut xcb_screen_t,
    pub rem: i32,
    pub index: i32,
}

#[repr(C)]
pub struct xcb_screen_t {
    pub root: xcb_window_t,
    pub default_colormap: xcb_colormap_t,
    pub white_pixel: u32,
    pub black_pixel: u32,
    pub current_input_masks: u32,
    pub width_in_pixels: u16,
    pub height_in_pixels: u16,
    pub width_in_millimeters: u16,
    pub height_in_millimeters: u16,
    pub min_installed_maps: u16,
    pub max_installed_maps: u16,
    pub root_visual: xcb_visualid_t,
    pub backing_stores: u8,
    pub save_unders: u8,
    pub root_depth: u8,
    pub allowed_depths_len: u8,
}

#[repr(C)]
pub struct xcb_randr_monitor_info_iterator_t {
    pub data: *mut xcb_randr_monitor_info_t,
    pub rem: i32,
    pub index: i32,
}

#[repr(C)]
pub struct xcb_randr_monitor_info_t {
    pub name: xcb_atom_t,
    pub primary: u8,
    pub automatic: u8,
    pub n_output: u16,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub width_mm: u32,
    pub height_mm: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct xcb_randr_get_monitors_cookie_t {
    pub sequence: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct xcb_shm_get_image_cookie_t {
    pub sequence: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct xcb_void_cookie_t {
    pub sequence: u32,
}

#[repr(C)]
pub struct xcb_generic_error_t {
    pub response_type: u8,
    pub error_code: u8,
    pub sequence: u16,
    pub resource_id: u32,
    pub minor_code: u16,
    pub major_code: u8,
    pub pad0: u8,
    pub pad: [u32; 5],
    pub full_sequence: u32,
}

#[repr(C)]
pub struct xcb_shm_get_image_reply_t {
    pub response_type: u8,
    pub depth: u8,
    pub sequence: u16,
    pub length: u32,
    pub visual: xcb_visualid_t,
    pub size: u32,
}

#[repr(C)]
pub struct xcb_randr_get_monitors_reply_t {
    pub response_type: u8,
    pub pad0: u8,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xcb_timestamp_t,
    pub n_monitors: u32,
    pub n_outputs: u32,
    pub pad1: [u8; 12],
}
