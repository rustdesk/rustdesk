#include "nv12_gl_texture.h"
#include "vaapi_prime_dec.h"

#include <epoxy/egl.h>
#include <epoxy/gl.h>
#include <string.h>
#include <unistd.h>

#include <mutex>
#include <unordered_map>
#include <vector>

#define NV12_GL_TEXTURE(obj)                                     \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), nv12_gl_texture_get_type(), \
                              Nv12GlTexture))

typedef struct _Nv12GlTexture Nv12GlTexture;
typedef struct _Nv12GlTextureClass Nv12GlTextureClass;

struct _Nv12GlTextureClass {
  FlTextureGLClass parent_class;
};

struct _Nv12GlTexture {
  FlTextureGL parent_instance;
  FlTextureRegistrar* texture_registrar;
  int64_t flutter_texture_id;
  GLuint rgba_tex;
  GLuint y_tex;
  GLuint uv_tex;
  GLuint program;
  GLuint fbo;
  GLuint vbo;
  GLint u_y;
  GLint u_uv;
  std::mutex* mu;
  std::vector<uint8_t>* y;
  std::vector<uint8_t>* uv;
  int y_stride;
  int uv_stride;
  int video_width;
  int video_height;
  int tex_width;
  int tex_height;
  gboolean ready;
  gboolean terminate;
  gboolean prime_ready;
  RustDeskPrimeFrame prime;
};

G_DEFINE_TYPE(Nv12GlTexture, nv12_gl_texture, fl_texture_gl_get_type())

static const char* kVs =
    "#version 120\n"
    "attribute vec2 a_pos;\n"
    "attribute vec2 a_uv;\n"
    "varying vec2 v_uv;\n"
    "void main() {\n"
    "  gl_Position = vec4(a_pos, 0.0, 1.0);\n"
    "  v_uv = a_uv;\n"
    "}\n";

static const char* kFs =
    "#version 120\n"
    "uniform sampler2D u_y;\n"
    "uniform sampler2D u_uv;\n"
    "varying vec2 v_uv;\n"
    "void main() {\n"
    "  float y = (texture2D(u_y, v_uv).r - 0.062745) * 1.164384;\n"
    "  vec2 uv = texture2D(u_uv, v_uv).rg - vec2(0.5);\n"
    "  float r = y + 1.596 * uv.y;\n"
    "  float g = y - 0.391 * uv.x - 0.813 * uv.y;\n"
    "  float b = y + 2.018 * uv.x;\n"
    "  gl_FragColor = vec4(r, g, b, 1.0);\n"
    "}\n";

static GLuint compile_shader(GLenum type, const char* src) {
  GLuint s = glCreateShader(type);
  glShaderSource(s, 1, &src, nullptr);
  glCompileShader(s);
  GLint ok = 0;
  glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
  if (!ok) {
    glDeleteShader(s);
    return 0;
  }
  return s;
}

static gboolean ensure_gl(Nv12GlTexture* self, int width, int height) {
  if (self->program == 0) {
    GLuint vs = compile_shader(GL_VERTEX_SHADER, kVs);
    GLuint fs = compile_shader(GL_FRAGMENT_SHADER, kFs);
    if (!vs || !fs) {
      if (vs) glDeleteShader(vs);
      if (fs) glDeleteShader(fs);
      return FALSE;
    }
    self->program = glCreateProgram();
    glAttachShader(self->program, vs);
    glAttachShader(self->program, fs);
    glBindAttribLocation(self->program, 0, "a_pos");
    glBindAttribLocation(self->program, 1, "a_uv");
    glLinkProgram(self->program);
    glDeleteShader(vs);
    glDeleteShader(fs);
    GLint ok = 0;
    glGetProgramiv(self->program, GL_LINK_STATUS, &ok);
    if (!ok) {
      glDeleteProgram(self->program);
      self->program = 0;
      return FALSE;
    }
    self->u_y = glGetUniformLocation(self->program, "u_y");
    self->u_uv = glGetUniformLocation(self->program, "u_uv");
    const float verts[] = {
        -1.f, -1.f, 0.f, 0.f, 1.f, -1.f, 1.f, 0.f, -1.f, 1.f, 0.f, 1.f,
        1.f,  1.f,  1.f, 1.f,
    };
    glGenBuffers(1, &self->vbo);
    glBindBuffer(GL_ARRAY_BUFFER, self->vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(verts), verts, GL_STATIC_DRAW);
    glGenTextures(1, &self->y_tex);
    glGenTextures(1, &self->uv_tex);
    glGenTextures(1, &self->rgba_tex);
    glGenFramebuffers(1, &self->fbo);
  }
  if (self->tex_width != width || self->tex_height != height) {
    glBindTexture(GL_TEXTURE_2D, self->rgba_tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, width, height, 0, GL_RGBA,
                 GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glBindFramebuffer(GL_FRAMEBUFFER, self->fbo);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D,
                           self->rgba_tex, 0);
    self->tex_width = width;
    self->tex_height = height;
  }
  return TRUE;
}

static void upload_plane(GLuint tex, int w, int h, int stride, GLenum fmt,
                         GLenum internal, const uint8_t* data) {
  glBindTexture(GL_TEXTURE_2D, tex);
  glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
  glPixelStorei(GL_UNPACK_ROW_LENGTH, stride);
  glTexImage2D(GL_TEXTURE_2D, 0, internal, w, h, 0, fmt, GL_UNSIGNED_BYTE,
               data);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
  glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
}

#ifndef EGL_LINUX_DMA_BUF_EXT
#define EGL_LINUX_DMA_BUF_EXT 0x3270
#endif
#ifndef EGL_LINUX_DRM_FOURCC_EXT
#define EGL_LINUX_DRM_FOURCC_EXT 0x3271
#endif
#ifndef EGL_DMA_BUF_PLANE0_FD_EXT
#define EGL_DMA_BUF_PLANE0_FD_EXT 0x3272
#define EGL_DMA_BUF_PLANE0_OFFSET_EXT 0x3273
#define EGL_DMA_BUF_PLANE0_PITCH_EXT 0x3274
#define EGL_DMA_BUF_PLANE1_FD_EXT 0x3275
#define EGL_DMA_BUF_PLANE1_OFFSET_EXT 0x3276
#define EGL_DMA_BUF_PLANE1_PITCH_EXT 0x3277
#define EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT 0x3443
#define EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT 0x3444
#define EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT 0x3445
#define EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT 0x3446
#endif
#ifndef EGL_DMA_BUF_PLANE2_FD_EXT
#define EGL_DMA_BUF_PLANE2_FD_EXT 0x3278
#define EGL_DMA_BUF_PLANE2_OFFSET_EXT 0x3279
#define EGL_DMA_BUF_PLANE2_PITCH_EXT 0x327A
#define EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT 0x3447
#define EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT 0x3448
#endif
#ifndef DRM_FORMAT_MOD_INVALID
#define DRM_FORMAT_MOD_INVALID 0x00ffffffffffffffULL
#endif

static PFNEGLCREATEIMAGEKHRPROC egl_create_image_khr() {
  return (PFNEGLCREATEIMAGEKHRPROC)eglGetProcAddress("eglCreateImageKHR");
}

static PFNEGLDESTROYIMAGEKHRPROC egl_destroy_image_khr() {
  return (PFNEGLDESTROYIMAGEKHRPROC)eglGetProcAddress("eglDestroyImageKHR");
}

static void destroy_image(EGLDisplay dpy, EGLImageKHR img) {
  if (img == EGL_NO_IMAGE_KHR) {
    return;
  }
  PFNEGLDESTROYIMAGEKHRPROC destroy = egl_destroy_image_khr();
  if (destroy) {
    destroy(dpy, img);
  } else {
    eglDestroyImage(dpy, img);
  }
}

static void close_prime(RustDeskPrimeFrame* p) {
  for (int i = 0; i < p->n_fds; i++) {
    if (p->fds[i] >= 0) {
      close(p->fds[i]);
      p->fds[i] = -1;
    }
  }
  p->n_fds = 0;
}

static const EGLint kPlaneFd[] = {
    EGL_DMA_BUF_PLANE0_FD_EXT,
    EGL_DMA_BUF_PLANE1_FD_EXT,
    EGL_DMA_BUF_PLANE2_FD_EXT,
};
static const EGLint kPlaneOff[] = {
    EGL_DMA_BUF_PLANE0_OFFSET_EXT,
    EGL_DMA_BUF_PLANE1_OFFSET_EXT,
    EGL_DMA_BUF_PLANE2_OFFSET_EXT,
};
static const EGLint kPlanePitch[] = {
    EGL_DMA_BUF_PLANE0_PITCH_EXT,
    EGL_DMA_BUF_PLANE1_PITCH_EXT,
    EGL_DMA_BUF_PLANE2_PITCH_EXT,
};
static const EGLint kPlaneModLo[] = {
    EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
    EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT,
    EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
};
static const EGLint kPlaneModHi[] = {
    EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
    EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
    EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT,
};

// dma-buf import is specified on eglCreateImageKHR + EGLint, not EGL 1.5
// EGLAttrib (64-bit pairs would misparse the list and yield a black texture).
static EGLImageKHR create_dmabuf_image(EGLDisplay dpy,
                                       const RustDeskPrimeFrame* p,
                                       uint32_t fourcc, int n_planes,
                                       gboolean with_modifier) {
  PFNEGLCREATEIMAGEKHRPROC create_image = egl_create_image_khr();
  if (!create_image) {
    g_warning("eglCreateImageKHR missing");
    return EGL_NO_IMAGE_KHR;
  }
  EGLint attrs[48];
  int i = 0;
  attrs[i++] = EGL_WIDTH;
  attrs[i++] = p->width;
  attrs[i++] = EGL_HEIGHT;
  attrs[i++] = p->height;
  attrs[i++] = EGL_LINUX_DRM_FOURCC_EXT;
  attrs[i++] = (EGLint)fourcc;
  int planes = n_planes;
  if (planes < 1) {
    planes = 1;
  }
  if (planes > 3) {
    planes = 3;
  }
  const gboolean use_mod =
      with_modifier && p->modifier != 0 && p->modifier != DRM_FORMAT_MOD_INVALID;
  for (int n = 0; n < planes; n++) {
    int obj = p->obj_indices[n];
    if (obj < 0 || obj >= p->n_fds) {
      obj = 0;
    }
    attrs[i++] = kPlaneFd[n];
    attrs[i++] = p->fds[obj];
    attrs[i++] = kPlaneOff[n];
    attrs[i++] = p->offsets[n];
    attrs[i++] = kPlanePitch[n];
    attrs[i++] = p->pitches[n];
    if (use_mod) {
      attrs[i++] = kPlaneModLo[n];
      attrs[i++] = (EGLint)(p->modifier & 0xffffffffu);
      attrs[i++] = kPlaneModHi[n];
      attrs[i++] = (EGLint)(p->modifier >> 32);
    }
  }
  attrs[i++] = EGL_NONE;
  EGLImageKHR img =
      create_image(dpy, EGL_NO_CONTEXT, EGL_LINUX_DMA_BUF_EXT, NULL, attrs);
  if (img == EGL_NO_IMAGE_KHR && use_mod) {
    img = create_dmabuf_image(dpy, p, fourcc, n_planes, FALSE);
  }
  return img;
}

static EGLImageKHR create_plane_image(EGLDisplay dpy, int fd, int w, int h,
                                      int pitch, int offset, uint32_t fourcc,
                                      uint64_t modifier) {
  RustDeskPrimeFrame tmp{};
  tmp.n_fds = 1;
  tmp.fds[0] = fd;
  tmp.fourcc = fourcc;
  tmp.modifier = modifier;
  tmp.width = w;
  tmp.height = h;
  tmp.n_planes = 1;
  tmp.pitches[0] = pitch;
  tmp.offsets[0] = offset;
  tmp.obj_indices[0] = 0;
  EGLImageKHR img = create_dmabuf_image(dpy, &tmp, fourcc, 1, TRUE);
  if (img == EGL_NO_IMAGE_KHR) {
    g_warning("eglCreateImageKHR fourcc=%#x %dx%d pitch=%d off=%d failed %#x",
              fourcc, w, h, pitch, offset, eglGetError());
  }
  return img;
}

static void draw_quad(Nv12GlTexture* self) {
  glBindBuffer(GL_ARRAY_BUFFER, self->vbo);
  glEnableVertexAttribArray(0);
  glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void*)0);
  glEnableVertexAttribArray(1);
  glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float),
                        (void*)(2 * sizeof(float)));
  glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
}

// mpv dmabuf_interop_gl: NV12 is two TEXTURE_2D images (R8 + GR88). Desktop
// GL binds with glEGLImageTargetTexStorageEXT (immutable); GLES with
// glEGLImageTargetTexture2DOES. Combined DRM_FORMAT_NV12 + EXTERNAL_OES is
// not how OpenGL samples planes.
static gboolean bind_plane_tex(GLuint* tex, EGLImageKHR img, gboolean desktop) {
  if (desktop) {
    if (*tex) {
      glDeleteTextures(1, tex);
    }
    glGenTextures(1, tex);
  }
  glBindTexture(GL_TEXTURE_2D, *tex);
  if (desktop) {
    auto storage = (PFNGLEGLIMAGETARGETTEXSTORAGEEXTPROC)eglGetProcAddress(
        "glEGLImageTargetTexStorageEXT");
    if (!storage) {
      g_warning("glEGLImageTargetTexStorageEXT missing (desktop GL)");
      return FALSE;
    }
    storage(GL_TEXTURE_2D, img, nullptr);
  } else {
    auto set_image = (PFNGLEGLIMAGETARGETTEXTURE2DOESPROC)eglGetProcAddress(
        "glEGLImageTargetTexture2DOES");
    if (!set_image) {
      g_warning("glEGLImageTargetTexture2DOES missing");
      return FALSE;
    }
    set_image(GL_TEXTURE_2D, img);
  }
  GLenum err = glGetError();
  if (err != GL_NO_ERROR) {
    g_warning("EGLImage→TEXTURE_2D desktop=%d err=%#x", desktop, err);
    return FALSE;
  }
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
  return TRUE;
}

static gboolean blit_prime(Nv12GlTexture* self, const RustDeskPrimeFrame* p) {
  static gboolean logged = FALSE;
  if (!ensure_gl(self, p->width, p->height) || p->n_planes < 2 ||
      p->n_fds < 1) {
    g_warning("blit_prime bad desc planes=%d fds=%d %dx%d fourcc=%#x",
              p->n_planes, p->n_fds, p->width, p->height, p->fourcc);
    return FALSE;
  }
  // dma-buf EGLImage must be created on the same EGL display as the current
  // GL context. Flutter Linux on X11 is GLX: eglGetCurrentDisplay() is empty,
  // and binding a default-display EGLImage into that GLX context SIGSEGVs
  // in glEGLImageTargetTexStorageEXT (seen: signal 11 in bind_plane_tex).
  EGLDisplay dpy = eglGetCurrentDisplay();
  if (dpy == EGL_NO_DISPLAY) {
    g_warning("blit_prime: Flutter GL is not EGL (likely GLX); skip dma-buf");
    return FALSE;
  }
  const char* egl_exts = eglQueryString(dpy, EGL_EXTENSIONS);
  if (!egl_exts || !strstr(egl_exts, "EGL_EXT_image_dma_buf_import")) {
    g_warning("EGL_EXT_image_dma_buf_import missing");
    return FALSE;
  }
  const gboolean desktop = epoxy_is_desktop_gl() ? TRUE : FALSE;
  if (!logged) {
    g_message(
        "PRIME frame fourcc=%#x %dx%d planes=%d fds=%d modifier=%#llx "
        "pitch0=%d off0=%d pitch1=%d off1=%d desktop_gl=%d",
        p->fourcc, p->width, p->height, p->n_planes, p->n_fds,
        (unsigned long long)p->modifier, p->pitches[0], p->offsets[0],
        p->pitches[1], p->offsets[1], desktop);
    logged = TRUE;
  }
  const uint32_t r8 = 0x20203852;    // DRM_FORMAT_R8
  const uint32_t gr88 = 0x38385247;  // DRM_FORMAT_GR88
  int i0 = p->obj_indices[0];
  int i1 = p->obj_indices[1];
  if (i0 < 0 || i0 >= p->n_fds) {
    i0 = 0;
  }
  if (i1 < 0 || i1 >= p->n_fds) {
    i1 = 0;
  }
  EGLImageKHR y_img =
      create_plane_image(dpy, p->fds[i0], p->width, p->height, p->pitches[0],
                         p->offsets[0], r8, p->modifier);
  EGLImageKHR uv_img = create_plane_image(
      dpy, p->fds[i1], p->width / 2, p->height / 2, p->pitches[1],
      p->offsets[1], gr88, p->modifier);
  if (y_img == EGL_NO_IMAGE_KHR || uv_img == EGL_NO_IMAGE_KHR) {
    destroy_image(dpy, y_img);
    destroy_image(dpy, uv_img);
    return FALSE;
  }
  gboolean ok = bind_plane_tex(&self->y_tex, y_img, desktop) &&
                bind_plane_tex(&self->uv_tex, uv_img, desktop);
  if (!ok) {
    destroy_image(dpy, y_img);
    destroy_image(dpy, uv_img);
    return FALSE;
  }

  glBindFramebuffer(GL_FRAMEBUFFER, self->fbo);
  glViewport(0, 0, p->width, p->height);
  glUseProgram(self->program);
  glActiveTexture(GL_TEXTURE0);
  glBindTexture(GL_TEXTURE_2D, self->y_tex);
  glUniform1i(self->u_y, 0);
  glActiveTexture(GL_TEXTURE1);
  glBindTexture(GL_TEXTURE_2D, self->uv_tex);
  glUniform1i(self->u_uv, 1);
  draw_quad(self);
  glBindFramebuffer(GL_FRAMEBUFFER, 0);
  destroy_image(dpy, y_img);
  destroy_image(dpy, uv_img);
  return TRUE;
}

static gboolean nv12_gl_texture_populate(FlTextureGL* texture, uint32_t* target,
                                         uint32_t* name, uint32_t* width,
                                         uint32_t* height, GError** error) {
  Nv12GlTexture* self = NV12_GL_TEXTURE(texture);
  int w = 0, h = 0, y_stride = 0, uv_stride = 0;
  std::vector<uint8_t> y;
  std::vector<uint8_t> uv;
  RustDeskPrimeFrame prime{};
  gboolean use_prime = FALSE;
  {
    std::lock_guard<std::mutex> lock(*self->mu);
    if (self->prime_ready && !self->terminate) {
      prime = self->prime;
      memset(&self->prime, 0, sizeof(self->prime));
      self->prime_ready = FALSE;
      use_prime = TRUE;
    } else if (!self->ready || self->terminate || self->video_width <= 0) {
      if (self->rgba_tex != 0) {
        *target = GL_TEXTURE_2D;
        *name = self->rgba_tex;
        *width = (uint32_t)self->tex_width;
        *height = (uint32_t)self->tex_height;
        return TRUE;
      }
      g_set_error(error, G_IO_ERROR, G_IO_ERROR_FAILED, "no nv12 frame");
      return FALSE;
    } else {
      w = self->video_width;
      h = self->video_height;
      y_stride = self->y_stride;
      uv_stride = self->uv_stride;
      y.swap(*self->y);
      uv.swap(*self->uv);
      self->ready = FALSE;
    }
  }
  if (use_prime) {
    gboolean ok = blit_prime(self, &prime);
    close_prime(&prime);
    if (!ok) {
      g_set_error(error, G_IO_ERROR, G_IO_ERROR_FAILED, "dma-buf import failed");
      return FALSE;
    }
    *target = GL_TEXTURE_2D;
    *name = self->rgba_tex;
    *width = (uint32_t)self->tex_width;
    *height = (uint32_t)self->tex_height;
    return TRUE;
  }
  if (!ensure_gl(self, w, h)) {
    g_set_error(error, G_IO_ERROR, G_IO_ERROR_FAILED, "nv12 gl init failed");
    return FALSE;
  }
  upload_plane(self->y_tex, w, h, y_stride, GL_RED, GL_R8, y.data());
  upload_plane(self->uv_tex, w / 2, h / 2, uv_stride / 2, GL_RG, GL_RG8,
               uv.data());

  glBindFramebuffer(GL_FRAMEBUFFER, self->fbo);
  glViewport(0, 0, w, h);
  glUseProgram(self->program);
  glActiveTexture(GL_TEXTURE0);
  glBindTexture(GL_TEXTURE_2D, self->y_tex);
  glUniform1i(self->u_y, 0);
  glActiveTexture(GL_TEXTURE1);
  glBindTexture(GL_TEXTURE_2D, self->uv_tex);
  glUniform1i(self->u_uv, 1);
  glBindBuffer(GL_ARRAY_BUFFER, self->vbo);
  glEnableVertexAttribArray(0);
  glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void*)0);
  glEnableVertexAttribArray(1);
  glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float),
                        (void*)(2 * sizeof(float)));
  glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
  glBindFramebuffer(GL_FRAMEBUFFER, 0);

  *target = GL_TEXTURE_2D;
  *name = self->rgba_tex;
  *width = (uint32_t)w;
  *height = (uint32_t)h;
  return TRUE;
}

static void nv12_gl_texture_dispose(GObject* object) {
  Nv12GlTexture* self = NV12_GL_TEXTURE(object);
  close_prime(&self->prime);
  delete self->mu;
  delete self->y;
  delete self->uv;
  self->mu = nullptr;
  self->y = nullptr;
  self->uv = nullptr;
  G_OBJECT_CLASS(nv12_gl_texture_parent_class)->dispose(object);
}

static void nv12_gl_texture_class_init(Nv12GlTextureClass* klass) {
  FL_TEXTURE_GL_CLASS(klass)->populate = nv12_gl_texture_populate;
  G_OBJECT_CLASS(klass)->dispose = nv12_gl_texture_dispose;
}

static void nv12_gl_texture_init(Nv12GlTexture* self) {
  self->mu = new std::mutex();
  self->y = new std::vector<uint8_t>();
  self->uv = new std::vector<uint8_t>();
}

typedef struct _Nv12GlTexturePlugin Nv12GlTexturePlugin;
typedef struct _Nv12GlTexturePluginClass Nv12GlTexturePluginClass;

struct _Nv12GlTexturePluginClass {
  GObjectClass parent_class;
};

struct _Nv12GlTexturePlugin {
  GObject parent_instance;
  FlTextureRegistrar* texture_registrar;
};

G_DEFINE_TYPE(Nv12GlTexturePlugin, nv12_gl_texture_plugin, G_TYPE_OBJECT)

static std::unordered_map<int64_t, Nv12GlTexture*> g_textures;

extern "C" __attribute__((visibility("default"))) void RustDeskNv12GlOnNv12(
    void* texture, const uint8_t* y,
                                     int y_stride, const uint8_t* uv,
                                     int uv_stride, int width, int height) {
  if (!texture || !y || !uv || width <= 0 || height <= 0 || y_stride < width ||
      uv_stride < width) {
    return;
  }
  Nv12GlTexture* self = NV12_GL_TEXTURE(texture);
  const size_t y_len = (size_t)y_stride * (size_t)height;
  const size_t uv_len = (size_t)uv_stride * (size_t)(height / 2);
  {
    std::lock_guard<std::mutex> lock(*self->mu);
    if (self->terminate) {
      return;
    }
    self->y->assign(y, y + y_len);
    self->uv->assign(uv, uv + uv_len);
    self->y_stride = y_stride;
    self->uv_stride = uv_stride;
    self->video_width = width;
    self->video_height = height;
    self->ready = TRUE;
  }
  fl_texture_registrar_mark_texture_frame_available(self->texture_registrar,
                                                    FL_TEXTURE(self));
}

extern "C" __attribute__((visibility("default"))) void RustDeskNv12GlOnPrime(
    void* texture, const RustDeskPrimeFrame* frame) {
  if (!texture || !frame || frame->kind != RUSTDESK_GPU_FRAME_PRIME ||
      frame->width <= 0 || frame->n_fds < 1) {
    return;
  }
  Nv12GlTexture* self = NV12_GL_TEXTURE(texture);
  RustDeskPrimeFrame copy = *frame;
  for (int i = 0; i < copy.n_fds; i++) {
    copy.fds[i] = dup(frame->fds[i]);
    if (copy.fds[i] < 0) {
      copy.n_fds = i;
      close_prime(&copy);
      return;
    }
  }
  {
    std::lock_guard<std::mutex> lock(*self->mu);
    if (self->terminate) {
      close_prime(&copy);
      return;
    }
    close_prime(&self->prime);
    self->prime = copy;
    self->prime_ready = TRUE;
    self->video_width = copy.width;
    self->video_height = copy.height;
  }
  fl_texture_registrar_mark_texture_frame_available(self->texture_registrar,
                                                    FL_TEXTURE(self));
}

static void plugin_handle(Nv12GlTexturePlugin* self, FlMethodCall* method_call) {
  const gchar* method = fl_method_call_get_name(method_call);
  g_autoptr(FlMethodResponse) response = nullptr;
  if (strcmp(method, "createTexture") == 0) {
    auto args = fl_method_call_get_args(method_call);
    int64_t key = fl_value_get_int(fl_value_lookup_string(args, "key"));
    if (g_textures.find(key) != g_textures.end()) {
      response = FL_METHOD_RESPONSE(
          fl_method_success_response_new(fl_value_new_int(-1)));
    } else {
      Nv12GlTexture* tex = NV12_GL_TEXTURE(
          g_object_new(nv12_gl_texture_get_type(), nullptr));
      tex->texture_registrar = self->texture_registrar;
      fl_texture_registrar_register_texture(self->texture_registrar,
                                            FL_TEXTURE(tex));
      int64_t id = reinterpret_cast<int64_t>(FL_TEXTURE(tex));
      tex->flutter_texture_id = id;
      g_textures[key] = tex;
      response = FL_METHOD_RESPONSE(
          fl_method_success_response_new(fl_value_new_int(id)));
    }
  } else if (strcmp(method, "closeTexture") == 0) {
    auto args = fl_method_call_get_args(method_call);
    int64_t key = fl_value_get_int(fl_value_lookup_string(args, "key"));
    auto it = g_textures.find(key);
    if (it != g_textures.end()) {
      {
        std::lock_guard<std::mutex> lock(*it->second->mu);
        it->second->terminate = TRUE;
      }
      fl_texture_registrar_unregister_texture(self->texture_registrar,
                                              FL_TEXTURE(it->second));
      g_object_unref(it->second);
      g_textures.erase(it);
    }
    response = FL_METHOD_RESPONSE(
        fl_method_success_response_new(fl_value_new_bool(true)));
  } else if (strcmp(method, "getTexturePtr") == 0) {
    auto args = fl_method_call_get_args(method_call);
    int64_t key = fl_value_get_int(fl_value_lookup_string(args, "key"));
    auto it = g_textures.find(key);
    int64_t ptr = it == g_textures.end()
                      ? 0
                      : static_cast<int64_t>(reinterpret_cast<size_t>(it->second));
    response = FL_METHOD_RESPONSE(
        fl_method_success_response_new(fl_value_new_int(ptr)));
  } else {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }
  fl_method_call_respond(method_call, response, nullptr);
}

static void method_cb(FlMethodChannel*, FlMethodCall* method_call,
                      gpointer user_data) {
  plugin_handle(G_TYPE_CHECK_INSTANCE_CAST(user_data, nv12_gl_texture_plugin_get_type(),
                                           Nv12GlTexturePlugin),
                method_call);
}

static void nv12_gl_texture_plugin_class_init(Nv12GlTexturePluginClass*) {}
static void nv12_gl_texture_plugin_init(Nv12GlTexturePlugin*) {}

void nv12_gl_texture_plugin_register_with_registrar(FlPluginRegistrar* registrar) {
  if (registrar == nullptr) {
    return;
  }
  Nv12GlTexturePlugin* plugin = static_cast<Nv12GlTexturePlugin*>(
      g_object_new(nv12_gl_texture_plugin_get_type(), nullptr));
  plugin->texture_registrar =
      fl_plugin_registrar_get_texture_registrar(registrar);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  g_autoptr(FlMethodChannel) channel = fl_method_channel_new(
      fl_plugin_registrar_get_messenger(registrar),
      "org.rustdesk.rustdesk/nv12_gl_texture", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, method_cb,
                                            g_object_ref(plugin),
                                            g_object_unref);
  g_object_unref(plugin);
}
