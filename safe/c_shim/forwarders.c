/*
 * Runtime sidecar resolver plus direct forwarders for transport-heavy symbols
 * that remain implemented by the reference library in impl-public-abi.
 */
#define _GNU_SOURCE 1
#include <dlfcn.h>
#include <libgen.h>
#include <limits.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CURL_ALLOW_OLD_MULTI_SOCKET 1
#include <curl/curl.h>
#include <curl/easy.h>
#include <curl/multi.h>
#include <curl/header.h>
#include <curl/websockets.h>

#ifdef curl_multi_socket
#undef curl_multi_socket
#endif

#ifndef REFERENCE_LIBRARY_FILE
#error "REFERENCE_LIBRARY_FILE must point at the sidecar reference library"
#endif

#ifndef REFERENCE_LIBRARY_ABSPATH
#error "REFERENCE_LIBRARY_ABSPATH must point at the absolute reference library path"
#endif

#ifndef BRIDGE_FLAVOR
#define BRIDGE_FLAVOR "unknown"
#endif

static pthread_once_t g_reference_once = PTHREAD_ONCE_INIT;
static void *g_reference_handle = NULL;

static void bridge_abort(const char *what, const char *detail) {
  fprintf(stderr, "libcurl transitional bridge (%s): %s: %s\n",
          BRIDGE_FLAVOR,
          what,
          detail ? detail : "(null)");
  abort();
}

static void bridge_open_reference(void) {
  Dl_info info;
  char self_path[PATH_MAX];
  char reference_path[PATH_MAX];
  const int flags = RTLD_NOW | RTLD_LOCAL;

  if(!dladdr((void *)&bridge_open_reference, &info) || !info.dli_fname)
    bridge_abort("dladdr", "could not resolve bridge location");

  if(snprintf(self_path, sizeof(self_path), "%s", info.dli_fname) <= 0)
    bridge_abort("snprintf", "failed to copy bridge path");

  if(snprintf(reference_path, sizeof(reference_path), "%s/%s",
              dirname(self_path),
              REFERENCE_LIBRARY_FILE) <= 0)
    bridge_abort("snprintf", "failed to compose reference path");

  g_reference_handle = dlopen(reference_path, flags);
  if(!g_reference_handle)
    g_reference_handle = dlopen(REFERENCE_LIBRARY_ABSPATH, flags);
  if(!g_reference_handle)
    bridge_abort("dlopen", dlerror());
}

static void *bridge_resolve_symbol(const char *name) {
  void *symbol = NULL;
  pthread_once(&g_reference_once, bridge_open_reference);
  symbol = dlsym(g_reference_handle, name);
  if(!symbol)
    bridge_abort("dlsym", name);
  return symbol;
}

void *curl_safe_resolve_reference_symbol(const char *name) {
  return bridge_resolve_symbol(name);
}

typedef CURLHcode (*curl_easy_header_fn)(CURL *easy,
                                         const char *name,
                                         size_t index,
                                         unsigned int origin,
                                         int request,
                                         struct curl_header **hout);

typedef struct curl_header *(*curl_easy_nextheader_fn)(CURL *easy,
                                                       unsigned int origin,
                                                       int request,
                                                       struct curl_header *prev);

typedef const char *(*curl_easy_strerror_fn)(CURLcode code);
typedef const char *(*curl_multi_strerror_fn)(CURLMcode code);

typedef char *(*curl_pushheader_byname_fn)(struct curl_pushheaders *h,
                                           const char *name);
typedef char *(*curl_pushheader_bynum_fn)(struct curl_pushheaders *h,
                                          size_t num);

typedef const struct curl_ws_frame *(*curl_ws_meta_fn)(CURL *curl);
typedef CURLcode (*curl_ws_recv_fn)(CURL *curl, void *buffer, size_t buflen,
                                    size_t *recv,
                                    const struct curl_ws_frame **metap);
typedef CURLcode (*curl_ws_send_fn)(CURL *curl, const void *buffer,
                                    size_t buflen, size_t *sent,
                                    curl_off_t fragsize,
                                    unsigned int flags);

#define RESOLVE_TYPED(name, type) ((type)bridge_resolve_symbol(name))

CURLHcode curl_easy_header(CURL *easy,
                           const char *name,
                           size_t index,
                           unsigned int origin,
                           int request,
                           struct curl_header **hout) {
  return RESOLVE_TYPED("curl_easy_header", curl_easy_header_fn)(
      easy, name, index, origin, request, hout);
}

struct curl_header *curl_easy_nextheader(CURL *easy,
                                         unsigned int origin,
                                         int request,
                                         struct curl_header *prev) {
  return RESOLVE_TYPED("curl_easy_nextheader", curl_easy_nextheader_fn)(
      easy, origin, request, prev);
}

const char *curl_easy_strerror(CURLcode code) {
  return RESOLVE_TYPED("curl_easy_strerror", curl_easy_strerror_fn)(code);
}

char *curl_pushheader_byname(struct curl_pushheaders *h, const char *name) {
  return RESOLVE_TYPED("curl_pushheader_byname", curl_pushheader_byname_fn)(
      h, name);
}

char *curl_pushheader_bynum(struct curl_pushheaders *h, size_t num) {
  return RESOLVE_TYPED("curl_pushheader_bynum", curl_pushheader_bynum_fn)(
      h, num);
}

const struct curl_ws_frame *curl_ws_meta(CURL *curl) {
  return RESOLVE_TYPED("curl_ws_meta", curl_ws_meta_fn)(curl);
}

CURLcode curl_ws_recv(CURL *curl,
                      void *buffer,
                      size_t buflen,
                      size_t *recv,
                      const struct curl_ws_frame **metap) {
  return RESOLVE_TYPED("curl_ws_recv", curl_ws_recv_fn)(curl, buffer, buflen,
                                                        recv, metap);
}

CURLcode curl_ws_send(CURL *curl,
                      const void *buffer,
                      size_t buflen,
                      size_t *sent,
                      curl_off_t fragsize,
                      unsigned int flags) {
  return RESOLVE_TYPED("curl_ws_send", curl_ws_send_fn)(curl, buffer, buflen,
                                                        sent, fragsize, flags);
}
