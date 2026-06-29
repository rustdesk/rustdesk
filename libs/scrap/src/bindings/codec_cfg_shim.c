#include <stdlib.h>

#define AOM_CODEC_USE_ENCODER 1
#define AOM_CODEC_USE_DECODER 1

#include <aom/aom_encoder.h>
#include <aom/aom_decoder.h>
#include <aom/aomcx.h>
#include <aom/aomdx.h>

#define VPX_CODEC_USE_ENCODER 1
#define VPX_CODEC_USE_DECODER 1

#include <vpx/vpx_encoder.h>
#include <vpx/vpx_decoder.h>
#include <vpx/vp8cx.h>
#include <vpx/vp8dx.h>

/* AOM encoder/decoder config helpers.
 *
 * Keep direct field access to aom_codec_*_cfg_t in C so Rust can treat those
 * bindgen-generated structs as opaque when newer libclang versions emit them
 * with only an _address field.
 */
aom_codec_err_t rustdesk_aom_enc_cfg_alloc_default(
    const aom_codec_iface_t* iface,
    unsigned int usage,
    aom_codec_enc_cfg_t** out
) {
    aom_codec_err_t result;

    if (!out) {
        return AOM_CODEC_INVALID_PARAM;
    }

    *out = (aom_codec_enc_cfg_t*)calloc(1, sizeof(aom_codec_enc_cfg_t));
    if (!*out) {
        return AOM_CODEC_MEM_ERROR;
    }

    result = aom_codec_enc_config_default(iface, *out, usage);
    if (result != AOM_CODEC_OK) {
        free(*out);
        *out = NULL;
    }

    return result;
}

void rustdesk_aom_enc_cfg_free(aom_codec_enc_cfg_t* cfg) {
    free(cfg);
}

void rustdesk_aom_enc_cfg_set_basic(
    aom_codec_enc_cfg_t* c,
    unsigned int w,
    unsigned int h,
    unsigned int threads,
    unsigned int bitrate,
    unsigned int timebase_den,
    unsigned int bit_depth,
    unsigned int usage_profile,
    unsigned int lag_in_frames
) {
    if (!c) {
        return;
    }

    c->g_w = w;
    c->g_h = h;
    c->g_threads = threads;
    c->g_timebase.num = 1;
    c->g_timebase.den = timebase_den;
    c->g_input_bit_depth = bit_depth;

    c->rc_target_bitrate = bitrate;
    c->rc_undershoot_pct = 50;
    c->rc_overshoot_pct = 50;
    c->rc_buf_initial_sz = 600;
    c->rc_buf_optimal_sz = 600;
    c->rc_buf_sz = 1000;

    c->g_usage = usage_profile;
    c->g_error_resilient = 0;
    c->rc_end_usage = AOM_CBR;
    c->g_pass = AOM_RC_ONE_PASS;
    c->g_lag_in_frames = lag_in_frames;
}

void rustdesk_aom_enc_cfg_set_quantizer(
    aom_codec_enc_cfg_t* c,
    unsigned int q_min,
    unsigned int q_max
) {
    if (!c) {
        return;
    }

    c->rc_min_quantizer = q_min;
    c->rc_max_quantizer = q_max;
}

void rustdesk_aom_enc_cfg_set_keyframe(
    aom_codec_enc_cfg_t* c,
    unsigned int min_dist,
    unsigned int max_dist,
    int disabled
) {
    if (!c) {
        return;
    }

    if (disabled) {
        c->kf_mode = AOM_KF_DISABLED;
    } else {
        c->kf_min_dist = min_dist;
        c->kf_max_dist = max_dist;
    }
}

void rustdesk_aom_enc_cfg_set_profile(
    aom_codec_enc_cfg_t* c,
    unsigned int profile
) {
    if (!c) {
        return;
    }

    c->g_profile = profile;
}

unsigned int rustdesk_aom_enc_cfg_get_w(const aom_codec_enc_cfg_t* c) {
    return c ? c->g_w : 0;
}

unsigned int rustdesk_aom_enc_cfg_get_h(const aom_codec_enc_cfg_t* c) {
    return c ? c->g_h : 0;
}

unsigned int rustdesk_aom_enc_cfg_get_threads(const aom_codec_enc_cfg_t* c) {
    return c ? c->g_threads : 0;
}

unsigned int rustdesk_aom_enc_cfg_get_target_bitrate(const aom_codec_enc_cfg_t* c) {
    return c ? c->rc_target_bitrate : 0;
}

aom_codec_err_t rustdesk_aom_dec_cfg_alloc(
    unsigned int threads,
    unsigned int w,
    unsigned int h,
    unsigned int allow_lowbitdepth,
    aom_codec_dec_cfg_t** out
) {
    if (!out) {
        return AOM_CODEC_INVALID_PARAM;
    }

    *out = (aom_codec_dec_cfg_t*)calloc(1, sizeof(aom_codec_dec_cfg_t));
    if (!*out) {
        return AOM_CODEC_MEM_ERROR;
    }

    (*out)->threads = threads;
    (*out)->w = w;
    (*out)->h = h;
    (*out)->allow_lowbitdepth = allow_lowbitdepth;

    return AOM_CODEC_OK;
}

void rustdesk_aom_dec_cfg_free(aom_codec_dec_cfg_t* cfg) {
    free(cfg);
}

/* VPX encoder/decoder config helpers. */
vpx_codec_err_t rustdesk_vpx_enc_cfg_alloc_default(
    const vpx_codec_iface_t* iface,
    unsigned int usage,
    vpx_codec_enc_cfg_t** out
) {
    vpx_codec_err_t result;

    if (!out) {
        return VPX_CODEC_INVALID_PARAM;
    }

    *out = (vpx_codec_enc_cfg_t*)calloc(1, sizeof(vpx_codec_enc_cfg_t));
    if (!*out) {
        return VPX_CODEC_MEM_ERROR;
    }

    result = vpx_codec_enc_config_default(iface, *out, usage);
    if (result != VPX_CODEC_OK) {
        free(*out);
        *out = NULL;
    }

    return result;
}

void rustdesk_vpx_enc_cfg_free(vpx_codec_enc_cfg_t* cfg) {
    free(cfg);
}

void rustdesk_vpx_enc_cfg_set_basic(
    vpx_codec_enc_cfg_t* c,
    unsigned int w,
    unsigned int h,
    unsigned int threads,
    unsigned int bitrate,
    unsigned int profile
) {
    if (!c) {
        return;
    }

    c->g_w = w;
    c->g_h = h;
    c->g_timebase.num = 1;
    c->g_timebase.den = 1000;
    c->rc_undershoot_pct = 95;
    c->rc_dropframe_thresh = 25;
    c->g_threads = threads;
    c->g_error_resilient = VPX_ERROR_RESILIENT_DEFAULT;
    c->rc_end_usage = VPX_CBR;
    c->rc_target_bitrate = bitrate;
    c->g_profile = profile;
}

void rustdesk_vpx_enc_cfg_set_quantizer(
    vpx_codec_enc_cfg_t* c,
    unsigned int q_min,
    unsigned int q_max
) {
    if (!c) {
        return;
    }

    c->rc_min_quantizer = q_min;
    c->rc_max_quantizer = q_max;
}

void rustdesk_vpx_enc_cfg_set_keyframe(
    vpx_codec_enc_cfg_t* c,
    unsigned int min_dist,
    unsigned int max_dist,
    int disabled
) {
    if (!c) {
        return;
    }

    if (disabled) {
        c->kf_mode = VPX_KF_DISABLED;
    } else {
        c->kf_min_dist = min_dist;
        c->kf_max_dist = max_dist;
    }
}

void rustdesk_vpx_enc_cfg_set_target_bitrate(
    vpx_codec_enc_cfg_t* c,
    unsigned int bitrate
) {
    if (!c) {
        return;
    }

    c->rc_target_bitrate = bitrate;
}

unsigned int rustdesk_vpx_enc_cfg_get_target_bitrate(const vpx_codec_enc_cfg_t* c) {
    return c ? c->rc_target_bitrate : 0;
}

vpx_codec_err_t rustdesk_vpx_dec_cfg_alloc(
    unsigned int threads,
    unsigned int w,
    unsigned int h,
    vpx_codec_dec_cfg_t** out
) {
    if (!out) {
        return VPX_CODEC_INVALID_PARAM;
    }

    *out = (vpx_codec_dec_cfg_t*)calloc(1, sizeof(vpx_codec_dec_cfg_t));
    if (!*out) {
        return VPX_CODEC_MEM_ERROR;
    }

    (*out)->threads = threads;
    (*out)->w = w;
    (*out)->h = h;

    return VPX_CODEC_OK;
}

void rustdesk_vpx_dec_cfg_free(vpx_codec_dec_cfg_t* cfg) {
    free(cfg);
}
