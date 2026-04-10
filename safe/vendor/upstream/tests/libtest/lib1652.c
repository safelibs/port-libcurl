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

struct debug_state {
  int saw_text;
  int saw_long_header;
  int bad_long_header;
};

static size_t discard_response(char *ptr, size_t size, size_t nmemb,
                               void *userdata)
{
  (void)ptr;
  (void)userdata;
  return size * nmemb;
}

static int debug_cb(CURL *handle, curl_infotype type, char *data, size_t size,
                    void *userdata)
{
  struct debug_state *state = userdata;
  size_t i;
  size_t value_len;

  (void)handle;

  if(type == CURLINFO_TEXT && size)
    state->saw_text = 1;

  if(type != CURLINFO_HEADER_IN)
    return 0;

  if(memcmp(data, "X-Long: ", 8))
    return 0;

  if(size <= 1024) {
    state->bad_long_header = 1;
    return 0;
  }

  value_len = size;
  while(value_len > 8 &&
        (data[value_len - 1] == '\r' || data[value_len - 1] == '\n'))
    value_len--;

  for(i = 8; i < value_len; ++i) {
    if(data[i] != 'A') {
      state->bad_long_header = 1;
      return 0;
    }
  }

  if(value_len == size) {
    state->bad_long_header = 1;
    return 0;
  }

  state->saw_long_header = 1;
  return 0;
}

int test(char *URL)
{
  CURL *curl = NULL;
  CURLcode res = TEST_ERR_MAJOR_BAD;
  struct debug_state state;

  memset(&state, 0, sizeof(state));

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    goto test_cleanup;
  }

  test_setopt(curl, CURLOPT_URL, URL);
  test_setopt(curl, CURLOPT_VERBOSE, 1L);
  test_setopt(curl, CURLOPT_DEBUGFUNCTION, debug_cb);
  test_setopt(curl, CURLOPT_DEBUGDATA, &state);
  test_setopt(curl, CURLOPT_WRITEFUNCTION, discard_response);
  test_setopt(curl, CURLOPT_PROXY, "");
  test_setopt(curl, CURLOPT_NOPROXY, "*");
  test_setopt(curl, CURLOPT_NOSIGNAL, 1L);

  res = curl_easy_perform(curl);
  if(res != CURLE_OK)
    goto test_cleanup;

  if(!state.saw_text) {
    fprintf(stderr, "verbose text callback was not exercised\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  if(!state.saw_long_header || state.bad_long_header) {
    fprintf(stderr, "long header callback verification failed\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

test_cleanup:
  curl_easy_cleanup(curl);
  curl_global_cleanup();
  return (int)res;
}
