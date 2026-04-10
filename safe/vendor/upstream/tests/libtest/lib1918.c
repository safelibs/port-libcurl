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

#include "testutil.h"
#include "warnless.h"
#include "memdebug.h"

static int contains_bytes(const char *haystack, size_t haystack_len,
                          const char *needle, size_t needle_len)
{
  size_t i;

  if(!needle_len || haystack_len < needle_len)
    return 0;

  for(i = 0; i <= haystack_len - needle_len; ++i) {
    if(!memcmp(&haystack[i], needle, needle_len))
      return 1;
  }
  return 0;
}

static int verify_handle_snapshot(CURL **handles, CURL *easy1, CURL *easy2,
                                  size_t expected_count,
                                  int expect_easy1, int expect_easy2)
{
  int seen_easy1 = 0;
  int seen_easy2 = 0;
  size_t i;
  size_t count = 0;

  if(!handles) {
    fprintf(stderr, "curl_multi_get_handles() returned NULL\n");
    return TEST_ERR_MAJOR_BAD;
  }

  for(i = 0; handles[i]; ++i) {
    ++count;
    if(handles[i] == easy1)
      ++seen_easy1;
    else if(handles[i] == easy2)
      ++seen_easy2;
    else {
      fprintf(stderr, "curl_multi_get_handles() returned an unknown handle\n");
      return TEST_ERR_MAJOR_BAD;
    }
  }

  if(count != expected_count) {
    fprintf(stderr, "curl_multi_get_handles() returned %u handles, "
            "expected %u\n", (unsigned int)count, (unsigned int)expected_count);
    return TEST_ERR_MAJOR_BAD;
  }

  if((seen_easy1 != expect_easy1) || (seen_easy2 != expect_easy2)) {
    fprintf(stderr, "curl_multi_get_handles() membership mismatch\n");
    return TEST_ERR_MAJOR_BAD;
  }

  return 0;
}

static int test_multi_get_handles_mode(void)
{
  CURLM *multi = NULL;
  CURL *easy1 = NULL;
  CURL *easy2 = NULL;
  CURL **handles = NULL;
  int easy1_added = 0;
  int easy2_added = 0;
  int res = 0;

  global_init(CURL_GLOBAL_ALL);
  multi_init(multi);
  easy_init(easy1);
  easy_init(easy2);

  handles = curl_multi_get_handles(multi);
  res = verify_handle_snapshot(handles, easy1, easy2, 0, 0, 0);
  if(res)
    goto test_cleanup;
  curl_free(handles);
  handles = NULL;
  printf("empty snapshot ok\n");

  multi_add_handle(multi, easy1);
  easy1_added = 1;
  multi_add_handle(multi, easy2);
  easy2_added = 1;

  handles = curl_multi_get_handles(multi);
  res = verify_handle_snapshot(handles, easy1, easy2, 2, 1, 1);
  if(res)
    goto test_cleanup;
  curl_free(handles);
  handles = NULL;
  printf("two handles listed\n");

  multi_remove_handle(multi, easy1);
  easy1_added = 0;

  handles = curl_multi_get_handles(multi);
  res = verify_handle_snapshot(handles, easy1, easy2, 1, 0, 1);
  if(res)
    goto test_cleanup;
  printf("removal updates snapshot\n");

test_cleanup:
  curl_free(handles);

  if(easy1_added)
    curl_multi_remove_handle(multi, easy1);
  if(easy2_added)
    curl_multi_remove_handle(multi, easy2);

  curl_easy_cleanup(easy1);
  curl_easy_cleanup(easy2);
  curl_multi_cleanup(multi);
  curl_global_cleanup();
  return res;
}

struct trace_ctx {
  int saw_tcp;
};

static size_t discard_body(char *ptr, size_t size, size_t nmemb, void *userdata)
{
  (void)ptr;
  (void)userdata;

  return size * nmemb;
}

static int debug_cb(CURL *easy, curl_infotype type, char *data,
                    size_t size, void *userp)
{
  struct trace_ctx *ctx = userp;
  (void)easy;

  if(type == CURLINFO_TEXT &&
     contains_bytes(data, size, "[TCP]", 5))
    ctx->saw_tcp = 1;

  return 0;
}

static int test_global_trace_mode(char *URL)
{
  CURL *easy = NULL;
  struct trace_ctx ctx;
  int res = 0;

  memset(&ctx, 0, sizeof(ctx));

  res = (int)curl_global_trace("-all");
  if(res) {
    fprintf(stderr, "curl_global_trace(\"-all\") failed\n");
    return res;
  }

  res = (int)curl_global_trace("unknown,+tcp");
  if(res) {
    fprintf(stderr, "curl_global_trace(\"unknown,+tcp\") failed\n");
    return res;
  }

  global_init(CURL_GLOBAL_ALL);
  easy_init(easy);
  easy_setopt(easy, CURLOPT_URL, URL);
  easy_setopt(easy, CURLOPT_PROXY, "");
  easy_setopt(easy, CURLOPT_NOPROXY, "*");
  easy_setopt(easy, CURLOPT_VERBOSE, 1L);
  easy_setopt(easy, CURLOPT_WRITEFUNCTION, discard_body);
  easy_setopt(easy, CURLOPT_DEBUGFUNCTION, debug_cb);
  easy_setopt(easy, CURLOPT_DEBUGDATA, &ctx);

  res = (int)curl_easy_perform(easy);
  if(res)
    goto test_cleanup;

  if(!ctx.saw_tcp) {
    fprintf(stderr, "curl_global_trace() did not enable TCP trace output\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  printf("tcp trace seen\n");

test_cleanup:
  curl_easy_cleanup(easy);
  curl_global_cleanup();
  return res;
}

int test(char *URL)
{
  const struct curl_easyoption *o;
  int error = 0;

  if(libtest_arg2) {
    if(!strcmp(libtest_arg2, "multi-get-handles"))
      return test_multi_get_handles_mode();
    if(!strcmp(libtest_arg2, "global-trace"))
      return test_global_trace_mode(URL);
    fprintf(stderr, "unknown lib1918 mode: %s\n", libtest_arg2);
    return TEST_ERR_USAGE;
  }

  curl_global_init(CURL_GLOBAL_ALL);

  for(o = curl_easy_option_next(NULL);
      o;
      o = curl_easy_option_next(o)) {
    const struct curl_easyoption *ename =
      curl_easy_option_by_name(o->name);
    const struct curl_easyoption *eid =
      curl_easy_option_by_id(o->id);

    if(ename->id != o->id) {
      printf("name lookup id %d doesn't match %d\n",
             ename->id, o->id);
    }
    else if(eid->id != o->id) {
      printf("ID lookup %d doesn't match %d\n",
             ename->id, o->id);
    }
  }
  curl_global_cleanup();
  return error;
}
