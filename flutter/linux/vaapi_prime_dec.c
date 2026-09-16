#include "vaapi_prime_dec.h"

#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_drm.h>
#include <libavutil/opt.h>
#include <string.h>
#include <unistd.h>

typedef struct VaapiPrimeDec {
  AVCodecContext* c;
  AVBufferRef* hw_device_ctx;
  AVFrame* frame;
  AVFrame* mapped;
  AVPacket* pkt;
  int hevc;
} VaapiPrimeDec;

static enum AVPixelFormat get_hw_format(AVCodecContext* ctx,
                                        const enum AVPixelFormat* pix_fmts) {
  (void)ctx;
  for (const enum AVPixelFormat* p = pix_fmts; *p != AV_PIX_FMT_NONE; p++) {
    if (*p == AV_PIX_FMT_VAAPI) {
      return *p;
    }
  }
  return AV_PIX_FMT_NONE;
}

static void close_mapped(VaapiPrimeDec* d) {
  if (d->mapped) {
    av_frame_unref(d->mapped);
  }
}

__attribute__((visibility("default"))) void* RustDeskVaapiPrimeOpen(int hevc) {
  VaapiPrimeDec* d = av_mallocz(sizeof(*d));
  if (!d) {
    return NULL;
  }
  d->hevc = hevc;
  const AVCodec* codec = avcodec_find_decoder_by_name(hevc ? "hevc" : "h264");
  if (!codec) {
    av_free(d);
    return NULL;
  }
  d->c = avcodec_alloc_context3(codec);
  if (!d->c) {
    av_free(d);
    return NULL;
  }
  d->c->flags |= AV_CODEC_FLAG_LOW_DELAY;
  d->c->thread_count = 1;
  d->c->get_format = get_hw_format;
  d->c->extra_hw_frames = 8;
  int ret = av_hwdevice_ctx_create(&d->hw_device_ctx, AV_HWDEVICE_TYPE_VAAPI,
                                   NULL, NULL, 0);
  if (ret < 0) {
    avcodec_free_context(&d->c);
    av_free(d);
    return NULL;
  }
  d->c->hw_device_ctx = av_buffer_ref(d->hw_device_ctx);
  d->pkt = av_packet_alloc();
  d->frame = av_frame_alloc();
  d->mapped = av_frame_alloc();
  if (!d->pkt || !d->frame || !d->mapped) {
    RustDeskVaapiPrimeClose(d);
    return NULL;
  }
  if (avcodec_open2(d->c, codec, NULL) != 0) {
    RustDeskVaapiPrimeClose(d);
    return NULL;
  }
  return d;
}

__attribute__((visibility("default"))) void RustDeskVaapiPrimeClose(void* ctx) {
  VaapiPrimeDec* d = ctx;
  if (!d) {
    return;
  }
  close_mapped(d);
  if (d->mapped) {
    av_frame_free(&d->mapped);
  }
  if (d->frame) {
    av_frame_free(&d->frame);
  }
  if (d->pkt) {
    av_packet_free(&d->pkt);
  }
  if (d->c) {
    avcodec_free_context(&d->c);
  }
  if (d->hw_device_ctx) {
    av_buffer_unref(&d->hw_device_ctx);
  }
  av_free(d);
}

static int fill_prime(AVFrame* mapped, RustDeskPrimeFrame* out) {
  AVDRMFrameDescriptor* desc = (AVDRMFrameDescriptor*)mapped->data[0];
  if (!desc || desc->nb_layers < 1 || desc->nb_objects < 1) {
    return 0;
  }
  memset(out, 0, sizeof(*out));
  out->kind = RUSTDESK_GPU_FRAME_PRIME;
  out->n_fds = desc->nb_objects;
  if (out->n_fds > 4) {
    out->n_fds = 4;
  }
  for (int i = 0; i < out->n_fds; i++) {
    int fd = dup(desc->objects[i].fd);
    if (fd < 0) {
      for (int j = 0; j < i; j++) {
        close(out->fds[j]);
      }
      return 0;
    }
    out->fds[i] = fd;
    if (i == 0) {
      out->modifier = desc->objects[i].format_modifier;
    }
  }
  out->width = mapped->width;
  out->height = mapped->height;
  // Mesa/AMD often exports NV12 as two layers (R8 + GR88), not one
  // NV12 layer with two planes. Flatten every layer's planes.
  int n = 0;
  for (int L = 0; L < desc->nb_layers && n < 4; L++) {
    AVDRMLayerDescriptor* layer = &desc->layers[L];
    for (int p = 0; p < layer->nb_planes && n < 4; p++) {
      out->pitches[n] = (int)layer->planes[p].pitch;
      out->offsets[n] = (int)layer->planes[p].offset;
      out->obj_indices[n] = layer->planes[p].object_index;
      n++;
    }
  }
  out->n_planes = n;
  if (desc->nb_layers == 1) {
    out->fourcc = desc->layers[0].format;
  } else {
    out->fourcc = 0x3231564e;  // DRM_FORMAT_NV12
  }
  return n >= 1;
}

__attribute__((visibility("default"))) int RustDeskVaapiPrimeDecode(
    void* ctx, const uint8_t* data, int len, RustDeskPrimeFrame* out) {
  VaapiPrimeDec* d = ctx;
  if (!d || !data || len <= 0 || !out) {
    return 0;
  }
  d->pkt->data = (uint8_t*)data;
  d->pkt->size = len;
  int ret = avcodec_send_packet(d->c, d->pkt);
  av_packet_unref(d->pkt);
  if (ret < 0) {
    return 0;
  }
  int got = 0;
  while (ret >= 0) {
    ret = avcodec_receive_frame(d->c, d->frame);
    if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
      break;
    }
    if (ret < 0) {
      break;
    }
    close_mapped(d);
    d->mapped->format = AV_PIX_FMT_DRM_PRIME;
    if (av_hwframe_map(d->mapped, d->frame, AV_HWFRAME_MAP_READ) < 0) {
      av_frame_unref(d->frame);
      continue;
    }
    if (fill_prime(d->mapped, out)) {
      got = 1;
    } else {
      close_mapped(d);
    }
    av_frame_unref(d->frame);
  }
  return got;
}
