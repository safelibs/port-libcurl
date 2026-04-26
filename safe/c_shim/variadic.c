#define CURL_DISABLE_TYPECHECK 1
#include <stdarg.h>
#include <stdint.h>
#include <string.h>

#include <curl/curl.h>
#include <curl/multi.h>
#include <curl/urlapi.h>

void *curl_safe_resolve_reference_symbol(const char *name);
CURL *curl_safe_reference_easy_handle(CURL *handle);
char *curl_safe_url_to_string(const CURLU *handle);

typedef CURLcode (*curl_easy_setopt_fn)(CURL *handle, CURLoption option, ...);
typedef CURLcode (*curl_easy_getinfo_fn)(CURL *handle, CURLINFO info, ...);
typedef CURLMcode (*curl_multi_setopt_fn)(CURLM *multi_handle, CURLMoption option, ...);

void curl_safe_easy_setopt_observe_long(CURL *handle, CURLoption option, long value);
void curl_safe_easy_setopt_observe_ptr(CURL *handle, CURLoption option, void *value);
void curl_safe_easy_setopt_observe_function(CURL *handle, CURLoption option, void (*value)(void));
void curl_safe_easy_setopt_observe_off_t(CURL *handle, CURLoption option, curl_off_t value);
CURLcode curl_safe_reference_easy_setopt_long(CURL *handle, CURLoption option, long value);
CURLSHcode curl_safe_share_setopt_int(CURLSH *share, CURLSHoption option, int value);
CURLSHcode curl_safe_share_setopt_function(CURLSH *share, CURLSHoption option, void (*value)(void));
CURLSHcode curl_safe_share_setopt_ptr(CURLSH *share, CURLSHoption option, void *value);
int curl_safe_easy_getinfo_string(CURL *handle, CURLINFO info, char **value, CURLcode *result);
int curl_safe_easy_getinfo_long(CURL *handle, CURLINFO info, long *value, CURLcode *result);
int curl_safe_easy_getinfo_off_t(CURL *handle, CURLINFO info, curl_off_t *value, CURLcode *result);
int curl_safe_easy_getinfo_socket(CURL *handle, CURLINFO info, curl_socket_t *value,
                                  CURLcode *result);
int curl_safe_easy_getinfo_ptr(CURL *handle, CURLINFO info, void **value, CURLcode *result);
CURLMcode curl_safe_multi_setopt_long(CURLM *multi_handle, CURLMoption option, long value);
CURLMcode curl_safe_multi_setopt_ptr(CURLM *multi_handle, CURLMoption option, void *value);
CURLMcode curl_safe_multi_setopt_function(CURLM *multi_handle, CURLMoption option, void (*value)(void));
CURLMcode curl_safe_multi_setopt_off_t(CURLM *multi_handle, CURLMoption option, curl_off_t value);
CURLMcode curl_safe_reference_multi_setopt_long(CURLM *multi_handle, CURLMoption option, long value);
CURLMcode curl_safe_reference_multi_setopt_ptr(CURLM *multi_handle, CURLMoption option, void *value);
CURLMcode curl_safe_reference_multi_setopt_function(CURLM *multi_handle, CURLMoption option, void (*value)(void));
CURLMcode curl_safe_reference_multi_setopt_off_t(CURLM *multi_handle, CURLMoption option, curl_off_t value);
CURLFORMcode curl_safe_formadd_parsed(struct curl_httppost **httppost,
                                      struct curl_httppost **last_post,
                                      const void *spec);

struct curl_safe_form_spec {
  const char *name;
  long namelength;
  const char *contents;
  long contentslength;
  const char *contenttype;
  struct curl_slist *contentheader;
  const char *filename;
  const char *filepath;
  const char *buffer_name;
  const char *buffer_ptr;
  size_t buffer_length;
  void *stream;
  curl_off_t contentlen;
  unsigned int flags;
};

#define FORM_FLAG_PTR_CONTENTS (1u << 0)
#define FORM_FLAG_FILE         (1u << 1)
#define FORM_FLAG_BUFFER       (1u << 2)
#define FORM_FLAG_TAKE_HEADERS (1u << 3)
#define FORM_FLAG_STREAM       (1u << 4)
#define FORM_FLAG_CONTENTLEN   (1u << 5)

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

CURLcode curl_safe_reference_easy_getinfo_slist(CURL *handle, CURLINFO info,
                                                struct curl_slist **value) {
  curl_easy_getinfo_fn fn = resolve_easy_getinfo();
  CURL *ref_handle = curl_safe_reference_easy_handle(handle);
  if(!ref_handle)
    return CURLE_BAD_FUNCTION_ARGUMENT;
  return fn(ref_handle, info, value);
}

CURLcode curl_safe_reference_easy_setopt_long(CURL *handle, CURLoption option, long value) {
  curl_easy_setopt_fn fn = resolve_easy_setopt();
  CURL *ref_handle = curl_safe_reference_easy_handle(handle);
  if(!ref_handle)
    return CURLE_BAD_FUNCTION_ARGUMENT;
  return fn(ref_handle, option, value);
}

static curl_multi_setopt_fn resolve_reference_multi_setopt(void) {
  static curl_multi_setopt_fn fn = NULL;
  if(!fn)
    fn = (curl_multi_setopt_fn)curl_safe_resolve_reference_symbol("curl_multi_setopt");
  return fn;
}

CURLcode curl_easy_setopt(CURL *handle, CURLoption option, ...) {
  CURLcode result;
  va_list args;
  long option_class = ((long)option) / 10000L;
  curl_easy_setopt_fn fn = resolve_easy_setopt();
  CURL *ref_handle = curl_safe_reference_easy_handle(handle);

  va_start(args, option);
  switch(option_class) {
  case 0:
  {
    long value = va_arg(args, long);
    result = ref_handle ? fn(ref_handle, option, value) : CURLE_OK;
    if(result == CURLE_OK)
      curl_safe_easy_setopt_observe_long(handle, option, value);
    break;
  }
  case 1:
  {
    void *value = va_arg(args, void *);
    if(option == CURLOPT_SHARE ||
       option == CURLOPT_COOKIEFILE ||
       option == CURLOPT_COOKIEJAR ||
       option == CURLOPT_COOKIELIST ||
       option == CURLOPT_HTTPPOST ||
       option == CURLOPT_MIMEPOST) {
      result = CURLE_OK;
      curl_safe_easy_setopt_observe_ptr(handle, option, value);
    }
    else if(option == CURLOPT_CURLU) {
      if(ref_handle) {
        char *url_text = value ? curl_safe_url_to_string((const CURLU *)value) : NULL;
        if(value && !url_text)
          result = CURLE_BAD_FUNCTION_ARGUMENT;
        else
          result = fn(ref_handle, CURLOPT_URL, url_text);
        if(url_text)
          curl_free(url_text);
      }
      else
        result = CURLE_OK;
      if(result == CURLE_OK)
        curl_safe_easy_setopt_observe_ptr(handle, option, value);
    }
    else {
      result = ref_handle ? fn(ref_handle, option, value) : CURLE_OK;
      if(result == CURLE_OK)
        curl_safe_easy_setopt_observe_ptr(handle, option, value);
    }
    break;
  }
  case 2:
  {
    void (*value)(void) = va_arg(args, void (*)(void));
    result = ref_handle ? fn(ref_handle, option, value) : CURLE_OK;
    if(result == CURLE_OK)
      curl_safe_easy_setopt_observe_function(handle, option, value);
    break;
  }
  case 3:
  {
    curl_off_t value = va_arg(args, curl_off_t);
    result = ref_handle ? fn(ref_handle, option, value) : CURLE_OK;
    if(result == CURLE_OK)
      curl_safe_easy_setopt_observe_off_t(handle, option, value);
    break;
  }
  case 4:
    result = ref_handle ? fn(ref_handle, option, va_arg(args, struct curl_blob *))
                        : CURLE_OK;
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
  CURL *ref_handle = curl_safe_reference_easy_handle(handle);

  va_start(args, info);
  switch(type_mask) {
  case CURLINFO_STRING:
  {
    char **value = va_arg(args, char **);
    if(!curl_safe_easy_getinfo_string(handle, info, value, &result))
      result = ref_handle ? fn(ref_handle, info, value) : CURLE_UNKNOWN_OPTION;
    break;
  }
  case CURLINFO_SLIST:
  {
    void **value = va_arg(args, void **);
    if(!curl_safe_easy_getinfo_ptr(handle, info, value, &result))
      result = ref_handle ? fn(ref_handle, info, value) : CURLE_UNKNOWN_OPTION;
    break;
  }
  case CURLINFO_LONG:
  {
    long *value = va_arg(args, long *);
    if(!curl_safe_easy_getinfo_long(handle, info, value, &result))
      result = ref_handle ? fn(ref_handle, info, value) : CURLE_UNKNOWN_OPTION;
    break;
  }
  case CURLINFO_DOUBLE:
    result = ref_handle ? fn(ref_handle, info, va_arg(args, double *))
                        : CURLE_UNKNOWN_OPTION;
    break;
  case CURLINFO_SOCKET:
  {
    curl_socket_t *value = va_arg(args, curl_socket_t *);
    if(!curl_safe_easy_getinfo_socket(handle, info, value, &result))
      result = ref_handle ? fn(ref_handle, info, value) : CURLE_UNKNOWN_OPTION;
    break;
  }
  case CURLINFO_OFF_T:
  {
    curl_off_t *value = va_arg(args, curl_off_t *);
    if(!curl_safe_easy_getinfo_off_t(handle, info, value, &result))
      result = ref_handle ? fn(ref_handle, info, value) : CURLE_UNKNOWN_OPTION;
    break;
  }
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

CURLMcode curl_safe_reference_multi_setopt_long(CURLM *multi_handle,
                                                CURLMoption option,
                                                long value) {
  curl_multi_setopt_fn fn = resolve_reference_multi_setopt();
  return fn(multi_handle, option, value);
}

CURLMcode curl_safe_reference_multi_setopt_ptr(CURLM *multi_handle,
                                               CURLMoption option,
                                               void *value) {
  curl_multi_setopt_fn fn = resolve_reference_multi_setopt();
  return fn(multi_handle, option, value);
}

CURLMcode curl_safe_reference_multi_setopt_function(CURLM *multi_handle,
                                                    CURLMoption option,
                                                    void (*value)(void)) {
  curl_multi_setopt_fn fn = resolve_reference_multi_setopt();
  return fn(multi_handle, option, value);
}

CURLMcode curl_safe_reference_multi_setopt_off_t(CURLM *multi_handle,
                                                 CURLMoption option,
                                                 curl_off_t value) {
  curl_multi_setopt_fn fn = resolve_reference_multi_setopt();
  return fn(multi_handle, option, value);
}

CURLSHcode curl_share_setopt(CURLSH *share, CURLSHoption option, ...) {
  CURLSHcode result;
  va_list args;

  va_start(args, option);
  switch(option) {
  case CURLSHOPT_SHARE:
  case CURLSHOPT_UNSHARE:
  {
    int value = va_arg(args, int);
    result = curl_safe_share_setopt_int(share, option, value);
    break;
  }
  case CURLSHOPT_LOCKFUNC:
  case CURLSHOPT_UNLOCKFUNC:
  {
    void (*value)(void) = va_arg(args, void (*)(void));
    result = curl_safe_share_setopt_function(share, option, value);
    break;
  }
  case CURLSHOPT_USERDATA:
  {
    void *value = va_arg(args, void *);
    result = curl_safe_share_setopt_ptr(share, option, value);
    break;
  }
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
  struct curl_safe_form_spec spec;
  va_list args;

  memset(&spec, 0, sizeof(spec));
  spec.namelength = -1;
  spec.contentslength = -1;
  spec.contentlen = -1;

  va_start(args, last_post);
  for(;;) {
    CURLformoption option = va_arg(args, CURLformoption);
    switch(option) {
    case CURLFORM_END:
      va_end(args);
      return curl_safe_formadd_parsed(httppost, last_post, &spec);
    case CURLFORM_COPYNAME:
    case CURLFORM_PTRNAME:
      if(spec.name) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.name = va_arg(args, const char *);
      break;
    case CURLFORM_NAMELENGTH:
      spec.namelength = va_arg(args, long);
      break;
    case CURLFORM_COPYCONTENTS:
      if(spec.contents || (spec.flags & FORM_FLAG_PTR_CONTENTS)) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.contents = va_arg(args, const char *);
      spec.flags &= ~FORM_FLAG_PTR_CONTENTS;
      break;
    case CURLFORM_PTRCONTENTS:
      if(spec.contents || (spec.flags & FORM_FLAG_PTR_CONTENTS)) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.contents = va_arg(args, const char *);
      spec.flags |= FORM_FLAG_PTR_CONTENTS;
      break;
    case CURLFORM_CONTENTSLENGTH:
      spec.contentslength = va_arg(args, long);
      break;
    case CURLFORM_CONTENTTYPE:
      if(spec.contenttype) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.contenttype = va_arg(args, const char *);
      break;
    case CURLFORM_CONTENTHEADER:
      if(spec.contentheader) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.contentheader = va_arg(args, struct curl_slist *);
      spec.flags |= FORM_FLAG_TAKE_HEADERS;
      break;
    case CURLFORM_FILE:
    case CURLFORM_FILECONTENT:
      if(spec.filepath) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.filepath = va_arg(args, const char *);
      spec.flags |= FORM_FLAG_FILE;
      break;
    case CURLFORM_FILENAME:
      if(spec.filename) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.filename = va_arg(args, const char *);
      break;
    case CURLFORM_BUFFER:
      if(spec.buffer_name) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.buffer_name = va_arg(args, const char *);
      spec.flags |= FORM_FLAG_BUFFER;
      break;
    case CURLFORM_BUFFERPTR:
      if(spec.buffer_ptr) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.buffer_ptr = va_arg(args, const char *);
      spec.flags |= FORM_FLAG_BUFFER;
      break;
    case CURLFORM_BUFFERLENGTH:
      spec.buffer_length = (size_t)va_arg(args, long);
      break;
    case CURLFORM_STREAM:
      if(spec.stream) {
        va_end(args);
        return CURL_FORMADD_OPTION_TWICE;
      }
      spec.stream = va_arg(args, void *);
      spec.flags |= FORM_FLAG_STREAM;
      break;
    case CURLFORM_CONTENTLEN:
      spec.contentlen = va_arg(args, curl_off_t);
      spec.flags |= FORM_FLAG_CONTENTLEN;
      break;
    case CURLFORM_ARRAY:
      (void)va_arg(args, const struct curl_forms *);
      va_end(args);
      return CURL_FORMADD_ILLEGAL_ARRAY;
    default:
      va_end(args);
      return CURL_FORMADD_UNKNOWN_OPTION;
    }
  }
}
