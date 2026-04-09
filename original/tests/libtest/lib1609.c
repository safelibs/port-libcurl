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

#ifdef HAVE_NETINET_IN_H
#include <netinet/in.h>
#endif
#ifdef HAVE_SYS_SOCKET_H
#include <sys/socket.h>
#endif

#include "memdebug.h"

#define TEST_HOST "replace.example"
#define TEST_PORT 1809

struct trace_data {
  char first_ip[64];
  bool saw_ipv4;
};

static curl_socket_t opensocket_cb(void *clientp,
                                   curlsocktype purpose,
                                   struct curl_sockaddr *address)
{
  struct trace_data *trace = clientp;
  struct sockaddr_in *ipv4;
  const unsigned char *octets;

  (void)purpose;

  if(trace && !trace->saw_ipv4 && address && (address->family == AF_INET)) {
    ipv4 = (struct sockaddr_in *)&address->addr;
    octets = (const unsigned char *)&ipv4->sin_addr;
    msnprintf(trace->first_ip, sizeof(trace->first_ip), "%u.%u.%u.%u",
              (unsigned int)octets[0], (unsigned int)octets[1],
              (unsigned int)octets[2], (unsigned int)octets[3]);
    trace->saw_ipv4 = TRUE;
  }

  return CURL_SOCKET_BAD;
}

static CURLcode init_share(CURLSH **share)
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

static CURLcode run_case(CURLSH *share, const char *entry,
                         struct trace_data *trace)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  struct curl_slist *resolve = NULL;
  char url[256];

  memset(trace, 0, sizeof(*trace));

  if(entry) {
    resolve = curl_slist_append(NULL, entry);
    if(!resolve) {
      fprintf(stderr, "curl_slist_append() failed\n");
      return TEST_ERR_MAJOR_BAD;
    }
  }

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    curl_slist_free_all(resolve);
    return TEST_ERR_EASY_INIT;
  }

  msnprintf(url, sizeof(url), "http://%s:%d/", TEST_HOST, TEST_PORT);

  res = curl_easy_setopt(curl, CURLOPT_URL, url);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_SHARE, share);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_DNS_CACHE_TIMEOUT, 60L);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_CONNECT_ONLY, 1L);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_FRESH_CONNECT, 1L);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_NOSIGNAL, 1L);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_PROXY, "");
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_NOPROXY, "*");
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_OPENSOCKETFUNCTION, opensocket_cb);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_OPENSOCKETDATA, trace);
  if(res != CURLE_OK)
    goto cleanup;

  if(resolve) {
    res = curl_easy_setopt(curl, CURLOPT_RESOLVE, resolve);
    if(res != CURLE_OK)
      goto cleanup;
  }

  res = curl_easy_perform(curl);

cleanup:
  curl_easy_cleanup(curl);
  curl_slist_free_all(resolve);
  return res;
}

int test(char *URL)
{
  CURLcode res = CURLE_OK;
  CURLSH *share = NULL;
  struct trace_data trace;
  (void)URL;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  res = init_share(&share);
  if(res)
    goto test_cleanup;

  res = run_case(share, "replace.example:1809:127.0.0.1", &trace);
  if(res != CURLE_COULDNT_CONNECT || !trace.saw_ipv4 ||
     strcmp(trace.first_ip, "127.0.0.1")) {
    fprintf(stderr, "first resolve entry failed\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = run_case(share, "replace.example:1809:127.0.0.2", &trace);
  if(res != CURLE_COULDNT_CONNECT || !trace.saw_ipv4 ||
     strcmp(trace.first_ip, "127.0.0.2")) {
    fprintf(stderr, "replacement resolve entry failed\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = run_case(share, NULL, &trace);
  if(res != CURLE_COULDNT_CONNECT || !trace.saw_ipv4 ||
     strcmp(trace.first_ip, "127.0.0.2")) {
    fprintf(stderr, "cached replacement entry was not reused\n");
    res = TEST_ERR_MAJOR_BAD;
    goto test_cleanup;
  }

  res = CURLE_OK;

test_cleanup:
  if(share)
    curl_share_cleanup(share);
  curl_global_cleanup();
  return (int)res;
}
