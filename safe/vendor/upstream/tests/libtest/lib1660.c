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

static size_t discard_cb(char *buffer, size_t size, size_t nitems,
                         void *userp)
{
  (void)buffer;
  (void)userp;
  return size * nitems;
}

static CURLcode do_store_request(const char *host, const char *hostip,
                                 const char *port, const char *hstsfile,
                                 const char *path)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  struct curl_slist *resolve = NULL;
  char url[256];
  char entry[256];

  msnprintf(url, sizeof(url), "https://%s:%s/%s", host, port, path);
  msnprintf(entry, sizeof(entry), "%s:%s:%s", host, port, hostip);

  resolve = curl_slist_append(NULL, entry);
  if(!resolve) {
    fprintf(stderr, "curl_slist_append() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    curl_slist_free_all(resolve);
    return TEST_ERR_EASY_INIT;
  }

  easy_setopt(curl, CURLOPT_URL, url);
  easy_setopt(curl, CURLOPT_HSTS, hstsfile);
  easy_setopt(curl, CURLOPT_HSTS_CTRL, CURLHSTS_ENABLE);
  easy_setopt(curl, CURLOPT_RESOLVE, resolve);
  easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
  easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, 0L);
  easy_setopt(curl, CURLOPT_PROXY, "");
  easy_setopt(curl, CURLOPT_NOPROXY, "*");
  easy_setopt(curl, CURLOPT_WRITEFUNCTION, discard_cb);

  res = curl_easy_perform(curl);

test_cleanup:
  curl_easy_cleanup(curl);
  curl_slist_free_all(resolve);
  return res;
}

static CURLcode do_reload_request(const char *port, const char *hstsfile,
                                  const char *path)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  char url[256];
  char *effective = NULL;
  static const char upgraded[] = "https://sub.goodhsts.example:";

  msnprintf(url, sizeof(url), "http://sub.goodhsts.example:%s/not-there/%s",
            port, path);

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    return TEST_ERR_EASY_INIT;
  }

  easy_setopt(curl, CURLOPT_URL, url);
  easy_setopt(curl, CURLOPT_HSTS, hstsfile);
  easy_setopt(curl, CURLOPT_HSTS_CTRL, CURLHSTS_ENABLE);
  easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
  easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, 0L);
  easy_setopt(curl, CURLOPT_PROXY, "");
  easy_setopt(curl, CURLOPT_NOPROXY, "*");
  easy_setopt(curl, CURLOPT_WRITEFUNCTION, discard_cb);

  res = curl_easy_perform(curl);
  if(res == CURLE_OK) {
    fprintf(stderr, "expected upgraded subdomain request to fail\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = curl_easy_getinfo(curl, CURLINFO_EFFECTIVE_URL, &effective);
  if(res != CURLE_OK)
    goto test_cleanup;

  if(!effective || strncmp(effective, upgraded, sizeof(upgraded) - 1)) {
    fprintf(stderr, "HSTS reload did not upgrade the subdomain URL\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  printf("%s\n", effective);
  res = CURLE_OK;

test_cleanup:
  curl_easy_cleanup(curl);
  return res;
}

static CURLcode do_expired_request(const char *port, const char *hstsfile,
                                   const char *path)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  char url[256];
  char *effective = NULL;
  static const char expected[] = "http://clear.example:";

  msnprintf(url, sizeof(url), "http://clear.example:%s/not-there/%s",
            port, path);

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    return TEST_ERR_EASY_INIT;
  }

  easy_setopt(curl, CURLOPT_URL, url);
  easy_setopt(curl, CURLOPT_HSTS, hstsfile);
  easy_setopt(curl, CURLOPT_HSTS_CTRL, CURLHSTS_ENABLE);
  easy_setopt(curl, CURLOPT_PROXY, "");
  easy_setopt(curl, CURLOPT_NOPROXY, "*");
  easy_setopt(curl, CURLOPT_WRITEFUNCTION, discard_cb);

  res = curl_easy_perform(curl);
  if(res == CURLE_OK) {
    fprintf(stderr, "expected expired clear.example request to fail\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = curl_easy_getinfo(curl, CURLINFO_EFFECTIVE_URL, &effective);
  if(res != CURLE_OK)
    goto test_cleanup;

  if(!effective || strncmp(effective, expected, sizeof(expected) - 1)) {
    fprintf(stderr, "expired HSTS entry still upgraded clear.example\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = CURLE_OK;

test_cleanup:
  curl_easy_cleanup(curl);
  return res;
}

int test(char *URL)
{
  CURLcode res = CURLE_OK;
  const char *hostip = libtest_arg2;
  const char *httpsport = libtest_arg3;
  const char *nlistenport = test_argc > 4 ? test_argv[4] : NULL;
  const char *hstsfile = test_argc > 5 ? test_argv[5] : NULL;
  char setpath[64];
  char addpath[64];

  if(!hostip || !httpsport || !nlistenport || !hstsfile) {
    fprintf(stderr, "Pass hostip, httpsport, nlistenport and hstsfile\n");
    return TEST_ERR_USAGE;
  }

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  msnprintf(setpath, sizeof(setpath), "%s0001", URL);
  res = do_store_request("clear.example", hostip, httpsport, hstsfile,
                         setpath);
  if(res)
    goto test_cleanup;

  wait_ms(1200);

  res = do_expired_request(nlistenport, hstsfile, URL);
  if(res)
    goto test_cleanup;

  msnprintf(addpath, sizeof(addpath), "%s0002", URL);
  res = do_store_request("goodhsts.example", hostip, httpsport, hstsfile,
                         addpath);
  if(res)
    goto test_cleanup;

  res = do_reload_request(nlistenport, hstsfile, URL);

test_cleanup:
  curl_global_cleanup();
  return (int)res;
}
