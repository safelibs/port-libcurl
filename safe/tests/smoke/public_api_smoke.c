#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <curl/curl.h>
#include <curl/easy.h>
#include <curl/mprintf.h>
#include <curl/multi.h>
#include <curl/options.h>
#include <curl/urlapi.h>

#define CHECK(cond, msg) do {                                             \
    if(!(cond)) {                                                         \
      fprintf(stderr, "%s:%d %s\n", __FILE__, __LINE__, (msg));           \
      rc = 1;                                                             \
      goto cleanup;                                                       \
    }                                                                     \
  } while(0)

struct tracked_ptr {
  void *ptr;
  struct tracked_ptr *next;
};

struct form_buffer {
  char data[4096];
  size_t len;
};

static struct tracked_ptr *g_tracked = NULL;

static void track_ptr(void *ptr) {
  struct tracked_ptr *node;
  if(!ptr)
    return;
  node = malloc(sizeof(*node));
  if(!node) {
    fprintf(stderr, "failed to track allocation\n");
    abort();
  }
  node->ptr = ptr;
  node->next = g_tracked;
  g_tracked = node;
}

static void untrack_ptr(void *ptr) {
  struct tracked_ptr **link = &g_tracked;
  while(*link) {
    if((*link)->ptr == ptr) {
      struct tracked_ptr *node = *link;
      *link = node->next;
      free(node);
      return;
    }
    link = &(*link)->next;
  }
}

static int is_tracked(void *ptr) {
  struct tracked_ptr *node = g_tracked;
  while(node) {
    if(node->ptr == ptr)
      return 1;
    node = node->next;
  }
  return 0;
}

static void *test_malloc(size_t size) {
  void *ptr = malloc(size);
  track_ptr(ptr);
  return ptr;
}

static void *test_calloc(size_t nmemb, size_t size) {
  void *ptr = calloc(nmemb, size);
  track_ptr(ptr);
  return ptr;
}

static void *test_realloc(void *ptr, size_t size) {
  void *new_ptr = realloc(ptr, size);
  if(!new_ptr)
    return NULL;
  if(ptr)
    untrack_ptr(ptr);
  track_ptr(new_ptr);
  return new_ptr;
}

static char *test_strdup(const char *input) {
  size_t len;
  char *copy;
  if(!input)
    return NULL;
  len = strlen(input) + 1;
  copy = malloc(len);
  if(!copy)
    return NULL;
  memcpy(copy, input, len);
  track_ptr(copy);
  return copy;
}

static void test_free(void *ptr) {
  if(!ptr)
    return;
  untrack_ptr(ptr);
  free(ptr);
}

static char *call_mvaprintf(const char *format, ...) {
  char *result;
  va_list args;
  va_start(args, format);
  result = curl_mvaprintf(format, args);
  va_end(args);
  return result;
}

static size_t append_form(void *arg, const char *buf, size_t len) {
  struct form_buffer *buffer = arg;
  size_t room = sizeof(buffer->data) - buffer->len - 1;
  if(len > room)
    len = room;
  memcpy(buffer->data + buffer->len, buf, len);
  buffer->len += len;
  buffer->data[buffer->len] = '\0';
  return len;
}

int main(void) {
  int rc = 0;
  CURL *easy = NULL;
  CURL *dup = NULL;
  CURLM *multi = NULL;
  CURLSH *share = NULL;
  CURLU *url = NULL;
  CURLU *urlcopy = NULL;
  curl_mime *mime = NULL;
  curl_mimepart *part = NULL;
  struct curl_slist *headers = NULL;
  struct curl_httppost *form = NULL;
  struct curl_httppost *last = NULL;
  struct form_buffer form_text = {{0}, 0};
  char *version = NULL;
  char *env_copy = NULL;
  char *formatted = NULL;
  char *formatted_va = NULL;
  char *escaped = NULL;
  char *unescaped = NULL;
  char *host = NULL;
  char *url_text = NULL;
  char *private_data = NULL;
  CURL **handles = NULL;
  int unescaped_len = 0;
  const struct curl_easyoption *option = NULL;
  const struct curl_easyoption *iter = NULL;
  size_t option_count = 0;
  curl_version_info_data *version_info = NULL;

  setenv("PORT_LIBCURL_SAFE_SMOKE_ENV", "smoke-value", 1);

  CHECK(curl_global_init_mem(CURL_GLOBAL_DEFAULT,
                             test_malloc,
                             test_free,
                             test_realloc,
                             test_strdup,
                             test_calloc) == CURLE_OK,
        "curl_global_init_mem() failed");

  version = curl_version();
  CHECK(version && is_tracked(version), "curl_version() did not use tracked storage");
  CHECK(version == curl_version(), "curl_version() did not cache");
  version_info = curl_version_info(CURLVERSION_NOW);
  CHECK(version_info && version_info->version, "curl_version_info() failed");

  CHECK(curl_strequal("AbC", "aBc") == 1, "curl_strequal() failed");
  CHECK(curl_strnequal("AbCd", "aBcE", 3) == 1, "curl_strnequal() failed");
  CHECK(curl_getdate("Sun, 06 Nov 1994 08:49:37 GMT", NULL) == 784111777,
        "curl_getdate() returned unexpected value");

  env_copy = curl_getenv("PORT_LIBCURL_SAFE_SMOKE_ENV");
  CHECK(env_copy && is_tracked(env_copy), "curl_getenv() did not use tracked storage");
  CHECK(strcmp(env_copy, "smoke-value") == 0, "curl_getenv() returned wrong data");
  curl_free(env_copy);
  env_copy = NULL;

  formatted = curl_maprintf("hello %s %d", "world", 7);
  CHECK(formatted && is_tracked(formatted), "curl_maprintf() failed");
  CHECK(strcmp(formatted, "hello world 7") == 0, "curl_maprintf() returned wrong text");
  curl_free(formatted);
  formatted = NULL;

  formatted_va = call_mvaprintf("value=%ld", 42L);
  CHECK(formatted_va && is_tracked(formatted_va), "curl_mvaprintf() failed");
  CHECK(strcmp(formatted_va, "value=42") == 0, "curl_mvaprintf() returned wrong text");
  curl_free(formatted_va);
  formatted_va = NULL;

  easy = curl_easy_init();
  CHECK(easy, "curl_easy_init() failed");
  CHECK(curl_easy_setopt(easy, CURLOPT_URL, "https://example.invalid/") == CURLE_OK,
        "curl_easy_setopt(CURLOPT_URL) failed");
  CHECK(curl_easy_setopt(easy, CURLOPT_PRIVATE, (void *)"public-api-smoke") == CURLE_OK,
        "curl_easy_setopt(CURLOPT_PRIVATE) failed");
  CHECK(curl_easy_getinfo(easy, CURLINFO_PRIVATE, &private_data) == CURLE_OK,
        "curl_easy_getinfo(CURLINFO_PRIVATE) failed");
  CHECK(private_data && strcmp(private_data, "public-api-smoke") == 0,
        "CURLINFO_PRIVATE mismatch");

  escaped = curl_easy_escape(easy, "a/b?c=d", 7);
  CHECK(escaped && is_tracked(escaped), "curl_easy_escape() failed");
  unescaped = curl_easy_unescape(easy, escaped, 0, &unescaped_len);
  CHECK(unescaped && is_tracked(unescaped), "curl_easy_unescape() failed");
  CHECK(unescaped_len == 7 && memcmp(unescaped, "a/b?c=d", 7) == 0,
        "curl_easy_unescape() returned wrong bytes");
  curl_free(unescaped);
  unescaped = NULL;
  curl_free(escaped);
  escaped = NULL;

  option = curl_easy_option_by_name("url");
  CHECK(option && strcmp(option->name, "URL") == 0, "curl_easy_option_by_name() failed");
  CHECK(option->type == CURLOT_STRING, "curl_easy_option_by_name() returned wrong type");
  CHECK(curl_easy_option_by_id(option->id) == option, "curl_easy_option_by_id() failed");
  for(iter = NULL; (iter = curl_easy_option_next(iter)) != NULL; )
    option_count++;
  CHECK(option_count > 100, "curl_easy_option_next() returned too few options");

  share = curl_share_init();
  CHECK(share, "curl_share_init() failed");
  CHECK(curl_share_setopt(share, CURLSHOPT_SHARE, CURL_LOCK_DATA_COOKIE) == CURLSHE_OK,
        "curl_share_setopt() failed");
  CHECK(curl_share_strerror(CURLSHE_OK) != NULL, "curl_share_strerror() failed");

  url = curl_url();
  CHECK(url, "curl_url() failed");
  CHECK(curl_url_set(url, CURLUPART_URL,
                     "https://user:pass@example.com:8443/base/path?x=1#frag",
                     0) == CURLUE_OK,
        "curl_url_set() failed");
  CHECK(curl_url_get(url, CURLUPART_HOST, &host, 0) == CURLUE_OK,
        "curl_url_get(host) failed");
  CHECK(host && is_tracked(host), "curl_url_get(host) did not use tracked storage");
  CHECK(strcmp(host, "example.com") == 0, "curl_url_get(host) returned wrong text");
  curl_free(host);
  host = NULL;

  urlcopy = curl_url_dup(url);
  CHECK(urlcopy, "curl_url_dup() failed");
  CHECK(curl_url_get(urlcopy, CURLUPART_URL, &url_text, 0) == CURLUE_OK,
        "curl_url_get(url) failed");
  CHECK(url_text && is_tracked(url_text), "curl_url_get(url) did not use tracked storage");
  CHECK(strncmp(url_text, "https://", 8) == 0, "curl_url_get(url) returned wrong text");
  curl_free(url_text);
  url_text = NULL;
  CHECK(curl_url_strerror(CURLUE_OK) != NULL, "curl_url_strerror() failed");

  headers = curl_slist_append(headers, "X-Test: public-api");
  CHECK(headers != NULL, "curl_slist_append() failed");

  mime = curl_mime_init(easy);
  CHECK(mime, "curl_mime_init() failed");
  part = curl_mime_addpart(mime);
  CHECK(part, "curl_mime_addpart() failed");
  CHECK(curl_mime_name(part, "field") == CURLE_OK, "curl_mime_name() failed");
  CHECK(curl_mime_data(part, "value", CURL_ZERO_TERMINATED) == CURLE_OK,
        "curl_mime_data() failed");

  CHECK(curl_formadd(&form, &last,
                     CURLFORM_COPYNAME, "field",
                     CURLFORM_COPYCONTENTS, "value",
                     CURLFORM_END) == CURL_FORMADD_OK,
        "curl_formadd() failed");
  CHECK(curl_formget(form, &form_text, append_form) == 0, "curl_formget() failed");
  CHECK(strstr(form_text.data, "field") && strstr(form_text.data, "value"),
        "curl_formget() returned unexpected payload");

  dup = curl_easy_duphandle(easy);
  CHECK(dup, "curl_easy_duphandle() failed");
  multi = curl_multi_init();
  CHECK(multi, "curl_multi_init() failed");
  CHECK(curl_multi_setopt(multi, CURLMOPT_MAXCONNECTS, 4L) == CURLM_OK,
        "curl_multi_setopt() failed");
  CHECK(curl_multi_add_handle(multi, dup) == CURLM_OK, "curl_multi_add_handle() failed");
  handles = curl_multi_get_handles(multi);
  CHECK(handles && is_tracked(handles), "curl_multi_get_handles() failed");
  CHECK(handles[0] == dup && handles[1] == NULL, "curl_multi_get_handles() returned wrong array");
  curl_free(handles);
  handles = NULL;
  CHECK(curl_multi_remove_handle(multi, dup) == CURLM_OK, "curl_multi_remove_handle() failed");
  CHECK(curl_multi_cleanup(multi) == CURLM_OK, "curl_multi_cleanup() failed");
  multi = NULL;

  curl_easy_reset(dup);

cleanup:
  if(handles)
    curl_free(handles);
  if(unescaped)
    curl_free(unescaped);
  if(escaped)
    curl_free(escaped);
  if(host)
    curl_free(host);
  if(url_text)
    curl_free(url_text);
  if(formatted)
    curl_free(formatted);
  if(formatted_va)
    curl_free(formatted_va);
  if(env_copy)
    curl_free(env_copy);
  curl_easy_cleanup(dup);
  if(easy)
    curl_easy_setopt(easy, CURLOPT_MIMEPOST, NULL);
  curl_easy_cleanup(easy);
  curl_mime_free(mime);
  curl_url_cleanup(urlcopy);
  curl_url_cleanup(url);
  curl_share_cleanup(share);
  curl_slist_free_all(headers);
  curl_formfree(form);
  curl_global_cleanup();

  if(version)
    CHECK(!is_tracked(version), "curl_global_cleanup() did not release curl_version() cache");
  return rc;
}
