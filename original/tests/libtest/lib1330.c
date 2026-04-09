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

#include <curl/urlapi.h>

#define MEMDEBUG_NODEFINES
#include "memdebug.h"

static size_t malloc_count;
static size_t free_count;
static size_t realloc_count;
static size_t strdup_count;
static size_t calloc_count;

static void *test_malloc(size_t size)
{
  ++malloc_count;
  return malloc(size);
}

static void test_free(void *ptr)
{
  if(ptr)
    ++free_count;
  free(ptr);
}

static void *test_realloc(void *ptr, size_t size)
{
  ++realloc_count;
  return realloc(ptr, size);
}

static char *test_strdup(const char *str)
{
  size_t len = strlen(str) + 1;
  char *copy = malloc(len);
  if(copy) {
    ++strdup_count;
    memcpy(copy, str, len);
  }
  return copy;
}

static void *test_calloc(size_t nmemb, size_t size)
{
  ++calloc_count;
  return calloc(nmemb, size);
}

int test(char *URL)
{
  CURL *easy = NULL;
  CURLU *url = NULL;
  CURLcode res;
  char *escaped = NULL;

  (void)URL;

  res = curl_global_init_mem(CURL_GLOBAL_ALL,
                             test_malloc,
                             test_free,
                             test_realloc,
                             test_strdup,
                             test_calloc);
  if(res != CURLE_OK) {
    fprintf(stderr, "curl_global_init_mem() failed: %d\n", (int)res);
    return TEST_ERR_MAJOR_BAD;
  }

  easy = curl_easy_init();
  if(!easy) {
    res = TEST_ERR_EASY_INIT;
    goto test_cleanup;
  }

  url = curl_url();
  if(!url || curl_url_set(url, CURLUPART_URL, "https://example.com/path", 0)) {
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = curl_easy_setopt(easy, CURLOPT_CURLU, url);
  if(res != CURLE_OK)
    goto test_cleanup;

  escaped = curl_easy_escape(easy, "needs escaping /", 16);
  if(!escaped) {
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  if((malloc_count + calloc_count + strdup_count) == 0 || free_count == 0) {
    fprintf(stderr, "custom memory callbacks were not exercised\n");
    res = TEST_ERR_MAJOR_BAD;
  }

test_cleanup:
  curl_free(escaped);
  curl_easy_cleanup(easy);
  curl_url_cleanup(url);
  curl_global_cleanup();
  return (int)res;
}
