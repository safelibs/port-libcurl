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

#define TEST_HOST "hash-create-destroy.invalid"

static CURLcode dns_share_init(CURLSH **share)
{
  CURLSHcode shres;

  *share = curl_share_init();
  if(!*share) {
    fprintf(stderr, "curl_share_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  shres = curl_share_setopt(*share, CURLSHOPT_SHARE, CURL_LOCK_DATA_DNS);
  if(shres != CURLSHE_OK) {
    fprintf(stderr, "curl_share_setopt(CURL_LOCK_DATA_DNS) failed\n");
    curl_share_cleanup(*share);
    *share = NULL;
    return TEST_ERR_MAJOR_BAD;
  }

  return CURLE_OK;
}

static CURLcode dns_share_cleanup(CURLSH *share)
{
  CURLSHcode shres;

  if(!share)
    return CURLE_OK;

  shres = curl_share_cleanup(share);
  if(shres != CURLSHE_OK) {
    fprintf(stderr, "curl_share_cleanup() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  return CURLE_OK;
}

static CURLcode perform_request(CURLSH *share, struct curl_slist *resolve,
                                const char *path)
{
  CURL *easy = NULL;
  CURLcode res = CURLE_OK;
  char target_url[256];

  easy = curl_easy_init();
  if(!easy) {
    fprintf(stderr, "curl_easy_init() failed\n");
    return TEST_ERR_EASY_INIT;
  }

  msnprintf(target_url, sizeof(target_url), "http://%s:%s/%s",
            TEST_HOST, libtest_arg3, path);

  res = curl_easy_setopt(easy, CURLOPT_URL, target_url);
  if(res != CURLE_OK)
    goto cleanup;

  res = curl_easy_setopt(easy, CURLOPT_SHARE, share);
  if(res != CURLE_OK)
    goto cleanup;

  if(resolve) {
    res = curl_easy_setopt(easy, CURLOPT_RESOLVE, resolve);
    if(res != CURLE_OK)
      goto cleanup;
  }

  res = curl_easy_perform(easy);

cleanup:
  curl_easy_cleanup(easy);
  return res;
}

int test(char *URL)
{
  CURLcode res = CURLE_OK;
  CURLSH *share = NULL;
  struct curl_slist *resolve = NULL;
  char dns_entry[256];

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  res = dns_share_init(&share);
  if(res)
    goto test_cleanup;

  if(strcmp(URL, "1305") == 0)
    goto test_cleanup;

  if(!libtest_arg2 || !libtest_arg3) {
    fprintf(stderr, "Pass address and port as arguments please\n");
    res = TEST_ERR_USAGE;
    goto test_cleanup;
  }

  msnprintf(dns_entry, sizeof(dns_entry), "%s:%s:%s",
            TEST_HOST, libtest_arg3, libtest_arg2);
  resolve = curl_slist_append(NULL, dns_entry);
  if(!resolve) {
    fprintf(stderr, "curl_slist_append() failed\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = perform_request(share, resolve, URL);

test_cleanup:
  curl_slist_free_all(resolve);
  if(!res)
    res = dns_share_cleanup(share);
  else
    (void)dns_share_cleanup(share);
  curl_global_cleanup();

  return (int)res;
}
