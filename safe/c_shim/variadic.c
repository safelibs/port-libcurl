#define CURL_DISABLE_TYPECHECK 1
#include <stdarg.h>

#include <curl/curl.h>
#include <curl/multi.h>

void *curl_safe_resolve_reference_symbol(const char *name);

typedef CURLcode (*curl_easy_setopt_fn)(CURL *handle, CURLoption option, ...);
typedef CURLcode (*curl_easy_getinfo_fn)(CURL *handle, CURLINFO info, ...);
typedef CURLSHcode (*curl_share_setopt_fn)(CURLSH *share, CURLSHoption option, ...);
typedef CURLFORMcode (*curl_formadd_fn)(struct curl_httppost **httppost,
                                        struct curl_httppost **last_post,
                                        ...);

void curl_safe_easy_setopt_observe_long(CURL *handle, CURLoption option, long value);
void curl_safe_easy_setopt_observe_ptr(CURL *handle, CURLoption option, void *value);
CURLMcode curl_safe_multi_setopt_long(CURLM *multi_handle, CURLMoption option, long value);
CURLMcode curl_safe_multi_setopt_ptr(CURLM *multi_handle, CURLMoption option, void *value);
CURLMcode curl_safe_multi_setopt_function(CURLM *multi_handle, CURLMoption option, void (*value)(void));
CURLMcode curl_safe_multi_setopt_off_t(CURLM *multi_handle, CURLMoption option, curl_off_t value);

static curl_easy_setopt_fn resolve_easy_setopt(void) {
  static curl_easy_setopt_fn fn = NULL;
  if(!fn)
    fn = (curl_easy_setopt_fn)curl_safe_resolve_reference_symbol("curl_easy_setopt");
  return fn;
}

static curl_easy_getinfo_fn resolve_easy_getinfo(void) {
  static curl_easy_getinfo_fn fn = NULL;
  if(!fn)
    fn = (curl_easy_getinfo_fn)curl_safe_resolve_reference_symbol("curl_easy_getinfo");
  return fn;
}

static curl_share_setopt_fn resolve_share_setopt(void) {
  static curl_share_setopt_fn fn = NULL;
  if(!fn)
    fn = (curl_share_setopt_fn)curl_safe_resolve_reference_symbol("curl_share_setopt");
  return fn;
}

static curl_formadd_fn resolve_formadd(void) {
  static curl_formadd_fn fn = NULL;
  if(!fn)
    fn = (curl_formadd_fn)curl_safe_resolve_reference_symbol("curl_formadd");
  return fn;
}

CURLcode curl_easy_setopt(CURL *handle, CURLoption option, ...) {
  CURLcode result;
  va_list args;
  long option_class = ((long)option) / 10000L;
  curl_easy_setopt_fn fn = resolve_easy_setopt();

  va_start(args, option);
  switch(option_class) {
  case 0:
  {
    long value = va_arg(args, long);
    result = fn(handle, option, value);
    if(result == CURLE_OK)
      curl_safe_easy_setopt_observe_long(handle, option, value);
    break;
  }
  case 1:
  {
    void *value = va_arg(args, void *);
    result = fn(handle, option, value);
    if(result == CURLE_OK)
      curl_safe_easy_setopt_observe_ptr(handle, option, value);
    break;
  }
  case 2:
    result = fn(handle, option, va_arg(args, void (*)(void)));
    break;
  case 3:
    result = fn(handle, option, va_arg(args, curl_off_t));
    break;
  case 4:
    result = fn(handle, option, va_arg(args, struct curl_blob *));
    break;
  default:
    result = CURLE_UNKNOWN_OPTION;
    break;
  }
  va_end(args);

  return result;
}

CURLcode curl_easy_getinfo(CURL *handle, CURLINFO info, ...) {
  CURLcode result;
  va_list args;
  unsigned int type_mask = info & CURLINFO_TYPEMASK;
  curl_easy_getinfo_fn fn = resolve_easy_getinfo();

  va_start(args, info);
  switch(type_mask) {
  case CURLINFO_STRING:
  case CURLINFO_SLIST:
    result = fn(handle, info, va_arg(args, void *));
    break;
  case CURLINFO_LONG:
    result = fn(handle, info, va_arg(args, long *));
    break;
  case CURLINFO_DOUBLE:
    result = fn(handle, info, va_arg(args, double *));
    break;
  case CURLINFO_SOCKET:
    result = fn(handle, info, va_arg(args, curl_socket_t *));
    break;
  case CURLINFO_OFF_T:
    result = fn(handle, info, va_arg(args, curl_off_t *));
    break;
  default:
    result = CURLE_UNKNOWN_OPTION;
    break;
  }
  va_end(args);

  return result;
}

CURLMcode curl_multi_setopt(CURLM *multi_handle, CURLMoption option, ...) {
  CURLMcode result;
  va_list args;
  long option_class = ((long)option) / 10000L;

  va_start(args, option);
  switch(option_class) {
  case 0:
    result = curl_safe_multi_setopt_long(multi_handle, option, va_arg(args, long));
    break;
  case 1:
    result = curl_safe_multi_setopt_ptr(multi_handle, option, va_arg(args, void *));
    break;
  case 2:
    result = curl_safe_multi_setopt_function(multi_handle, option, va_arg(args, void (*)(void)));
    break;
  case 3:
    result = curl_safe_multi_setopt_off_t(multi_handle, option, va_arg(args, curl_off_t));
    break;
  default:
    result = CURLM_UNKNOWN_OPTION;
    break;
  }
  va_end(args);

  return result;
}

CURLSHcode curl_share_setopt(CURLSH *share, CURLSHoption option, ...) {
  CURLSHcode result;
  va_list args;
  curl_share_setopt_fn fn = resolve_share_setopt();

  va_start(args, option);
  switch(option) {
  case CURLSHOPT_SHARE:
  case CURLSHOPT_UNSHARE:
    result = fn(share, option, va_arg(args, int));
    break;
  case CURLSHOPT_LOCKFUNC:
  case CURLSHOPT_UNLOCKFUNC:
    result = fn(share, option, va_arg(args, void (*)(void)));
    break;
  case CURLSHOPT_USERDATA:
    result = fn(share, option, va_arg(args, void *));
    break;
  default:
    result = CURLSHE_BAD_OPTION;
    break;
  }
  va_end(args);

  return result;
}

CURLFORMcode curl_formadd(struct curl_httppost **httppost,
                          struct curl_httppost **last_post,
                          ...) {
#if defined(__GNUC__) || defined(__clang__)
  curl_formadd_fn fn = resolve_formadd();
  void *args = __builtin_apply_args();
  void *ret = __builtin_apply((void (*)())fn, args, 4096);
  __builtin_return(ret);
#else
  (void)httppost;
  (void)last_post;
  return CURL_FORMADD_DISABLED;
#endif
}
