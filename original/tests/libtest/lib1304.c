/***************************************************************************
 *                                  _   _ ____  _
 *  Project                     ___| | | |  _ \| |
 *                             / __| | | | |_) | |
 *                            | (__| |_| |  _ <| |___
 *                             \___|\___/|_| \_\_____|
 *
 * Copyright (C) Daniel Stenberg, <daniel@haxx.se>, et al.
 *
 * This software is licensed as described in the file COPYING, which
 * you should have received as part of this distribution. The terms
 * are also available at https://curl.se/docs/copyright.html.
 *
 * You may opt to use, copy, modify, merge, publish, distribute and/or sell
 * copies of the Software, and permit persons to whom the Software is
 * furnished to do so, under the terms of the COPYING file.
 *
 * This software is distributed on an "AS IS" basis, WITHOUT WARRANTY OF ANY
 * KIND, either express or implied.
 *
 * SPDX-License-Identifier: curl
 *
 ***************************************************************************/

#include "test.h"

#include "memdebug.h"

#define FIRST_HOST "netrc1304.example"
#define SECOND_HOST "curl1304.example"
#define MISSING_HOST "missing1304.example"

#define AUTH_ADMI "YWRtaTo="
#define AUTH_ADMINN "YWRtaW5uOg=="
#define AUTH_ADMIN "YWRtaW46cGFzc3dk"
#define AUTH_NONE "bm9uZTpub25l"

struct transfer_state {
  bool saw_auth;
  char auth[64];
};

struct netrc_case {
  const char *name;
  const char *host;
  const char *userinfo;
  const char *expected_auth;
};

static size_t discard_cb(char *ptr, size_t size, size_t nmemb, void *userdata)
{
  (void)ptr;
  (void)userdata;
  return size * nmemb;
}

static int debug_cb(CURL *handle, curl_infotype type, char *data, size_t size,
                    void *userdata)
{
  struct transfer_state *state = userdata;
  static const char prefix[] = "Authorization: Basic ";
  size_t pos = 0;
  size_t prefixlen = sizeof(prefix) - 1;

  (void)handle;

  if(type != CURLINFO_HEADER_OUT)
    return 0;

  while(pos < size) {
    const char *line = data + pos;
    size_t linelen = 0;

    while((pos + linelen) < size && data[pos + linelen] != '\n')
      linelen++;

    if(linelen && line[linelen - 1] == '\r')
      linelen--;

    if(linelen >= prefixlen && !memcmp(line, prefix, prefixlen)) {
      const char *value = line + prefixlen;
      size_t valuelen = linelen - prefixlen;

      if(valuelen >= sizeof(state->auth))
        valuelen = sizeof(state->auth) - 1;

      memcpy(state->auth, value, valuelen);
      state->auth[valuelen] = '\0';
      state->saw_auth = true;
      break;
    }

    pos += linelen;
    if(pos < size && data[pos] == '\r')
      pos++;
    if(pos < size && data[pos] == '\n')
      pos++;
  }

  return 0;
}

static CURLcode parse_base_url(const char *url, char **scheme, char **host,
                               char **port)
{
  CURLU *uh = NULL;
  CURLUcode uc;
  CURLcode result = CURLE_URL_MALFORMAT;

  *scheme = NULL;
  *host = NULL;
  *port = NULL;

  uh = curl_url();
  if(!uh)
    return CURLE_OUT_OF_MEMORY;

  uc = curl_url_set(uh, CURLUPART_URL, url, 0);
  if(uc)
    goto cleanup;

  uc = curl_url_get(uh, CURLUPART_SCHEME, scheme, 0);
  if(uc)
    goto cleanup;

  uc = curl_url_get(uh, CURLUPART_HOST, host, 0);
  if(uc)
    goto cleanup;

  uc = curl_url_get(uh, CURLUPART_PORT, port, CURLU_DEFAULT_PORT);
  if(uc)
    goto cleanup;

  result = CURLE_OK;

cleanup:
  if(result != CURLE_OK) {
    curl_free(*scheme);
    curl_free(*host);
    curl_free(*port);
    *scheme = NULL;
    *host = NULL;
    *port = NULL;
  }

  curl_url_cleanup(uh);
  return result;
}

static CURLcode append_resolve_entry(struct curl_slist **resolve,
                                     const char *host, const char *port,
                                     const char *address)
{
  char entry[256];
  struct curl_slist *updated;

  msnprintf(entry, sizeof(entry), "%s:%s:%s", host, port, address);
  updated = curl_slist_append(*resolve, entry);
  if(!updated)
    return CURLE_OUT_OF_MEMORY;

  *resolve = updated;
  return CURLE_OK;
}

static CURLcode run_case(const char *scheme, const char *port,
                         const char *netrc_file, struct curl_slist *resolve,
                         const struct netrc_case *testcase)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  struct transfer_state state;
  char url[256];

  memset(&state, 0, sizeof(state));

  if(testcase->userinfo) {
    msnprintf(url, sizeof(url), "%s://%s%s:%s/", scheme,
              testcase->userinfo, testcase->host, port);
  }
  else {
    msnprintf(url, sizeof(url), "%s://%s:%s/", scheme,
              testcase->host, port);
  }

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "%s: curl_easy_init() failed\n", testcase->name);
    return TEST_ERR_EASY_INIT;
  }

  test_setopt(curl, CURLOPT_URL, url);
  test_setopt(curl, CURLOPT_NETRC, (long)CURL_NETRC_REQUIRED);
  test_setopt(curl, CURLOPT_NETRC_FILE, netrc_file);
  test_setopt(curl, CURLOPT_VERBOSE, 1L);
  test_setopt(curl, CURLOPT_DEBUGFUNCTION, debug_cb);
  test_setopt(curl, CURLOPT_DEBUGDATA, &state);
  test_setopt(curl, CURLOPT_WRITEFUNCTION, discard_cb);
  test_setopt(curl, CURLOPT_RESOLVE, resolve);
  test_setopt(curl, CURLOPT_PROXY, "");
  test_setopt(curl, CURLOPT_NOPROXY, "*");
  test_setopt(curl, CURLOPT_NOSIGNAL, 1L);

  res = curl_easy_perform(curl);
  if(res != CURLE_OK) {
    fprintf(stderr, "%s: curl_easy_perform() failed: %d\n",
            testcase->name, res);
    goto test_cleanup;
  }

  if(testcase->expected_auth) {
    if(!state.saw_auth) {
      fprintf(stderr, "%s: expected an Authorization header\n",
              testcase->name);
      res = TEST_ERR_FAILURE;
      goto test_cleanup;
    }

    if(strcmp(state.auth, testcase->expected_auth)) {
      fprintf(stderr, "%s: unexpected Authorization header: %s\n",
              testcase->name, state.auth);
      res = TEST_ERR_FAILURE;
      goto test_cleanup;
    }
  }
  else if(state.saw_auth) {
    fprintf(stderr, "%s: unexpected Authorization header: %s\n",
            testcase->name, state.auth);
    res = TEST_ERR_FAILURE;
  }

test_cleanup:
  curl_easy_cleanup(curl);
  return res;
}

int test(char *URL)
{
  static const struct netrc_case cases[] = {
    { "host miss", MISSING_HOST, NULL, NULL },
    { "substring login miss", FIRST_HOST, "admi@", AUTH_ADMI },
    { "superstring login miss", FIRST_HOST, "adminn@", AUTH_ADMINN },
    { "first host fallback", FIRST_HOST, NULL, AUTH_ADMIN },
    { "second host fallback", SECOND_HOST, NULL, AUTH_NONE }
  };
  CURLcode res = CURLE_OK;
  struct curl_slist *resolve = NULL;
  char *scheme = NULL;
  char *address = NULL;
  char *port = NULL;
  size_t i;

  if(!libtest_arg2) {
    fprintf(stderr, "missing .netrc file argument\n");
    return TEST_ERR_USAGE;
  }

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  res = parse_base_url(URL, &scheme, &address, &port);
  if(res != CURLE_OK) {
    fprintf(stderr, "failed to parse base URL: %s\n", URL);
    goto cleanup;
  }

  res = append_resolve_entry(&resolve, FIRST_HOST, port, address);
  if(res != CURLE_OK)
    goto cleanup;

  res = append_resolve_entry(&resolve, SECOND_HOST, port, address);
  if(res != CURLE_OK)
    goto cleanup;

  res = append_resolve_entry(&resolve, MISSING_HOST, port, address);
  if(res != CURLE_OK)
    goto cleanup;

  for(i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
    res = run_case(scheme, port, libtest_arg2, resolve, &cases[i]);
    if(res != CURLE_OK)
      goto cleanup;
  }

cleanup:
  curl_slist_free_all(resolve);
  curl_free(scheme);
  curl_free(address);
  curl_free(port);
  curl_global_cleanup();

  return (int)res;
}
