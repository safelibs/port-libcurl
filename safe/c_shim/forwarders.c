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

typedef CURLcode (*curl_easy_pause_fn)(CURL *handle, int bitmask);
typedef CURLcode (*curl_easy_perform_fn)(CURL *curl);
typedef CURLcode (*curl_easy_recv_fn)(CURL *curl, void *buffer, size_t buflen,
                                      size_t *n);
typedef CURLcode (*curl_easy_send_fn)(CURL *curl, const void *buffer,
                                      size_t buflen, size_t *n);
typedef const char *(*curl_easy_strerror_fn)(CURLcode code);
typedef CURLcode (*curl_easy_upkeep_fn)(CURL *curl);

typedef CURLMcode (*curl_multi_add_handle_fn)(CURLM *multi_handle,
                                              CURL *curl_handle);
typedef CURLMcode (*curl_multi_assign_fn)(CURLM *multi_handle,
                                          curl_socket_t sockfd,
                                          void *sockp);
typedef CURLMcode (*curl_multi_cleanup_fn)(CURLM *multi_handle);
typedef CURLMcode (*curl_multi_fdset_fn)(CURLM *multi_handle,
                                         fd_set *read_fd_set,
                                         fd_set *write_fd_set,
                                         fd_set *exc_fd_set,
                                         int *max_fd);
typedef CURLMsg *(*curl_multi_info_read_fn)(CURLM *multi_handle,
                                            int *msgs_in_queue);
typedef CURLM *(*curl_multi_init_fn)(void);
typedef CURLMcode (*curl_multi_perform_fn)(CURLM *multi_handle,
                                           int *running_handles);
typedef CURLMcode (*curl_multi_poll_fn)(CURLM *multi_handle,
                                        struct curl_waitfd extra_fds[],
                                        unsigned int extra_nfds,
                                        int timeout_ms,
                                        int *ret);
typedef CURLMcode (*curl_multi_remove_handle_fn)(CURLM *multi_handle,
                                                 CURL *curl_handle);
typedef CURLMcode (*curl_multi_socket_fn)(CURLM *multi_handle,
                                          curl_socket_t s,
                                          int *running_handles);
typedef CURLMcode (*curl_multi_socket_action_fn)(CURLM *multi_handle,
                                                 curl_socket_t s,
                                                 int ev_bitmask,
                                                 int *running_handles);
typedef CURLMcode (*curl_multi_socket_all_fn)(CURLM *multi_handle,
                                              int *running_handles);
typedef const char *(*curl_multi_strerror_fn)(CURLMcode code);
typedef CURLMcode (*curl_multi_timeout_fn)(CURLM *multi_handle,
                                           long *milliseconds);
typedef CURLMcode (*curl_multi_wait_fn)(CURLM *multi_handle,
                                        struct curl_waitfd extra_fds[],
                                        unsigned int extra_nfds,
                                        int timeout_ms,
                                        int *ret);
typedef CURLMcode (*curl_multi_wakeup_fn)(CURLM *multi_handle);

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

CURLcode curl_easy_pause(CURL *handle, int bitmask) {
  return RESOLVE_TYPED("curl_easy_pause", curl_easy_pause_fn)(handle, bitmask);
}

CURLcode curl_easy_perform(CURL *curl) {
  return RESOLVE_TYPED("curl_easy_perform", curl_easy_perform_fn)(curl);
}

CURLcode curl_easy_recv(CURL *curl, void *buffer, size_t buflen, size_t *n) {
  return RESOLVE_TYPED("curl_easy_recv", curl_easy_recv_fn)(curl, buffer,
                                                            buflen, n);
}

CURLcode curl_easy_send(CURL *curl, const void *buffer, size_t buflen,
                        size_t *n) {
  return RESOLVE_TYPED("curl_easy_send", curl_easy_send_fn)(curl, buffer,
                                                            buflen, n);
}

const char *curl_easy_strerror(CURLcode code) {
  return RESOLVE_TYPED("curl_easy_strerror", curl_easy_strerror_fn)(code);
}

CURLcode curl_easy_upkeep(CURL *curl) {
  return RESOLVE_TYPED("curl_easy_upkeep", curl_easy_upkeep_fn)(curl);
}

CURLMcode curl_multi_add_handle(CURLM *multi_handle, CURL *curl_handle) {
  return RESOLVE_TYPED("curl_multi_add_handle", curl_multi_add_handle_fn)(
      multi_handle, curl_handle);
}

CURLMcode curl_multi_assign(CURLM *multi_handle,
                            curl_socket_t sockfd,
                            void *sockp) {
  return RESOLVE_TYPED("curl_multi_assign", curl_multi_assign_fn)(
      multi_handle, sockfd, sockp);
}

CURLMcode curl_multi_cleanup(CURLM *multi_handle) {
  return RESOLVE_TYPED("curl_multi_cleanup", curl_multi_cleanup_fn)(
      multi_handle);
}

CURLMcode curl_multi_fdset(CURLM *multi_handle,
                           fd_set *read_fd_set,
                           fd_set *write_fd_set,
                           fd_set *exc_fd_set,
                           int *max_fd) {
  return RESOLVE_TYPED("curl_multi_fdset", curl_multi_fdset_fn)(
      multi_handle, read_fd_set, write_fd_set, exc_fd_set, max_fd);
}

CURLMsg *curl_multi_info_read(CURLM *multi_handle, int *msgs_in_queue) {
  return RESOLVE_TYPED("curl_multi_info_read", curl_multi_info_read_fn)(
      multi_handle, msgs_in_queue);
}

CURLM *curl_multi_init(void) {
  return RESOLVE_TYPED("curl_multi_init", curl_multi_init_fn)();
}

CURLMcode curl_multi_perform(CURLM *multi_handle, int *running_handles) {
  return RESOLVE_TYPED("curl_multi_perform", curl_multi_perform_fn)(
      multi_handle, running_handles);
}

CURLMcode curl_multi_poll(CURLM *multi_handle,
                          struct curl_waitfd extra_fds[],
                          unsigned int extra_nfds,
                          int timeout_ms,
                          int *ret) {
  return RESOLVE_TYPED("curl_multi_poll", curl_multi_poll_fn)(
      multi_handle, extra_fds, extra_nfds, timeout_ms, ret);
}

CURLMcode curl_multi_remove_handle(CURLM *multi_handle, CURL *curl_handle) {
  return RESOLVE_TYPED("curl_multi_remove_handle",
                       curl_multi_remove_handle_fn)(multi_handle, curl_handle);
}

CURLMcode curl_multi_socket(CURLM *multi_handle,
                            curl_socket_t s,
                            int *running_handles) {
  return RESOLVE_TYPED("curl_multi_socket", curl_multi_socket_fn)(
      multi_handle, s, running_handles);
}

CURLMcode curl_multi_socket_action(CURLM *multi_handle,
                                   curl_socket_t s,
                                   int ev_bitmask,
                                   int *running_handles) {
  return RESOLVE_TYPED("curl_multi_socket_action",
                       curl_multi_socket_action_fn)(multi_handle, s,
                                                    ev_bitmask,
                                                    running_handles);
}

CURLMcode curl_multi_socket_all(CURLM *multi_handle, int *running_handles) {
  return RESOLVE_TYPED("curl_multi_socket_all", curl_multi_socket_all_fn)(
      multi_handle, running_handles);
}

const char *curl_multi_strerror(CURLMcode code) {
  return RESOLVE_TYPED("curl_multi_strerror", curl_multi_strerror_fn)(code);
}

CURLMcode curl_multi_timeout(CURLM *multi_handle, long *milliseconds) {
  return RESOLVE_TYPED("curl_multi_timeout", curl_multi_timeout_fn)(
      multi_handle, milliseconds);
}

CURLMcode curl_multi_wait(CURLM *multi_handle,
                          struct curl_waitfd extra_fds[],
                          unsigned int extra_nfds,
                          int timeout_ms,
                          int *ret) {
  return RESOLVE_TYPED("curl_multi_wait", curl_multi_wait_fn)(
      multi_handle, extra_fds, extra_nfds, timeout_ms, ret);
}

CURLMcode curl_multi_wakeup(CURLM *multi_handle) {
  return RESOLVE_TYPED("curl_multi_wakeup", curl_multi_wakeup_fn)(
      multi_handle);
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
