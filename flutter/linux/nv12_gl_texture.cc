#include "nv12_gl_texture.h"

#include <epoxy/gl.h>
#include <string.h>

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
    "  float y = texture2D(u_y, v_uv).r;\n"
    "  vec2 uv = texture2D(u_uv, v_uv).rg - vec2(0.5);\n"
    "  float r = y + 1.402 * uv.y;\n"
    "  float g = y - 0.344136 * uv.x - 0.714136 * uv.y;\n"
    "  float b = y + 1.772 * uv.x;\n"
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
        -1.f, -1.f, 0.f, 1.f, 1.f, -1.f, 1.f, 1.f, -1.f, 1.f, 0.f, 0.f,
        1.f,  1.f,  1.f, 0.f,
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
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
  glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
}

static gboolean nv12_gl_texture_populate(FlTextureGL* texture, uint32_t* target,
                                         uint32_t* name, uint32_t* width,
                                         uint32_t* height, GError** error) {
  Nv12GlTexture* self = NV12_GL_TEXTURE(texture);
  int w = 0, h = 0, y_stride = 0, uv_stride = 0;
  std::vector<uint8_t> y;
  std::vector<uint8_t> uv;
  {
    std::lock_guard<std::mutex> lock(*self->mu);
    if (!self->ready || self->terminate || self->video_width <= 0) {
      if (self->rgba_tex != 0) {
        *target = GL_TEXTURE_2D;
        *name = self->rgba_tex;
        *width = (uint32_t)self->tex_width;
        *height = (uint32_t)self->tex_height;
        return TRUE;
      }
      g_set_error(error, G_IO_ERROR, G_IO_ERROR_FAILED, "no nv12 frame");
      return FALSE;
    }
    w = self->video_width;
    h = self->video_height;
    y_stride = self->y_stride;
    uv_stride = self->uv_stride;
    y.swap(*self->y);
    uv.swap(*self->uv);
    self->ready = FALSE;
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
