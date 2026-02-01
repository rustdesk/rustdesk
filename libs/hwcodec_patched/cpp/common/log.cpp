
#include "log.h"
extern "C" {
  #include <libavutil/log.h>
}
namespace gol {
enum {
  LOG_LEVEL_ERROR = 0,
  LOG_LEVEL_WARN = 1,
  LOG_LEVEL_INFO = 2,
  LOG_LEVEL_DEBUG = 3,
  LOG_LEVEL_TRACE = 4,
};

extern "C" void hwcodec_log(int level, const char *message);
extern "C" void hwcodec_av_log_callback(int level, const char *message);

void log_to_rust(int level, const std::string &message) {
  const char *cstr = message.c_str();
  hwcodec_log(level, cstr);
}

void error(const std::string &message) {
  log_to_rust(LOG_LEVEL_ERROR, message);
}

void warn(const std::string &message) { log_to_rust(LOG_LEVEL_WARN, message); }

void info(const std::string &message) { log_to_rust(LOG_LEVEL_INFO, message); }

void debug(const std::string &message) {
  log_to_rust(LOG_LEVEL_DEBUG, message);
}

void trace(const std::string &message) {
  log_to_rust(LOG_LEVEL_TRACE, message);
}

void av_log_callback(void *ptr, int level, const char *fmt, va_list vl) {
  (void)ptr;
  if (level > av_log_get_level()) {
    return;
  }
  char line[1024] = {0};
  vsnprintf(line, sizeof(line), fmt, vl);
  hwcodec_av_log_callback(level, line);
};

} // namespace gol


extern "C" void hwcodec_set_av_log_callback() {
  av_log_set_callback(gol::av_log_callback);
}
