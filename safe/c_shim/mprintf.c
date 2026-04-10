#include <stdarg.h>
#include <stdio.h>

#include <curl/mprintf.h>

void *curl_safe_malloc(size_t size);
void curl_safe_free(void *ptr);

static char *curl_vaprintf_alloc(const char *format, va_list args) {
  va_list measure;
  va_list render;
  int needed;
  char *buffer;

  va_copy(measure, args);
  needed = vsnprintf(NULL, 0, format, measure);
  va_end(measure);
  if(needed < 0)
    return NULL;

  buffer = curl_safe_malloc((size_t)needed + 1);
  if(!buffer)
    return NULL;

  va_copy(render, args);
  if(vsnprintf(buffer, (size_t)needed + 1, format, render) < 0) {
    va_end(render);
    curl_safe_free(buffer);
    return NULL;
  }
  va_end(render);

  return buffer;
}

int curl_mprintf(const char *format, ...) {
  int rc;
  va_list args;
  va_start(args, format);
  rc = vprintf(format, args);
  va_end(args);
  return rc;
}

int curl_mfprintf(FILE *fd, const char *format, ...) {
  int rc;
  va_list args;
  va_start(args, format);
  rc = vfprintf(fd, format, args);
  va_end(args);
  return rc;
}

int curl_msprintf(char *buffer, const char *format, ...) {
  int rc;
  va_list args;
  va_start(args, format);
  rc = vsprintf(buffer, format, args);
  va_end(args);
  return rc;
}

int curl_msnprintf(char *buffer, size_t maxlength, const char *format, ...) {
  int rc;
  va_list args;
  va_start(args, format);
  rc = vsnprintf(buffer, maxlength, format, args);
  va_end(args);
  return rc;
}

int curl_mvprintf(const char *format, va_list args) {
  return vprintf(format, args);
}

int curl_mvfprintf(FILE *fd, const char *format, va_list args) {
  return vfprintf(fd, format, args);
}

int curl_mvsprintf(char *buffer, const char *format, va_list args) {
  return vsprintf(buffer, format, args);
}

int curl_mvsnprintf(char *buffer, size_t maxlength, const char *format, va_list args) {
  return vsnprintf(buffer, maxlength, format, args);
}

char *curl_maprintf(const char *format, ...) {
  char *buffer;
  va_list args;
  va_start(args, format);
  buffer = curl_vaprintf_alloc(format, args);
  va_end(args);
  return buffer;
}

char *curl_mvaprintf(const char *format, va_list args) {
  return curl_vaprintf_alloc(format, args);
}
