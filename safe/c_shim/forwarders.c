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

typedef const char *(*curl_easy_strerror_fn)(CURLcode code);
typedef const char *(*curl_multi_strerror_fn)(CURLMcode code);

#define RESOLVE_TYPED(name, type) ((type)bridge_resolve_symbol(name))

const char *curl_easy_strerror(CURLcode code) {
  return RESOLVE_TYPED("curl_easy_strerror", curl_easy_strerror_fn)(code);
}
