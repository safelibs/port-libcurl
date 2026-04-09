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

#ifndef CURL_DISABLE_SHUFFLE_DNS

#ifdef HAVE_NETINET_IN_H
#include <netinet/in.h>
#endif
#ifdef HAVE_SYS_SOCKET_H
#include <sys/socket.h>
#endif

#include "memdebug.h"

#define NUM_ATTEMPTS 10
#define TEST_PORT 8999
#define SHUFFLE_ADDRS "127.0.0.1,127.0.0.2,127.0.0.3,127.0.0.4," \
                      "127.0.0.5,127.0.0.6,127.0.0.7,127.0.0.8"

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

static CURLcode resolve_once(int attempt,
                             char *primary_ip, size_t primary_ip_size)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  struct curl_slist *resolve = NULL;
  char url[128];
  char host[64];
  char entry[192];
  struct trace_data trace;

  memset(&trace, 0, sizeof(trace));
  msnprintf(host, sizeof(host), "shuffle-%d.example", attempt);
  msnprintf(url, sizeof(url), "http://%s:%d/", host, TEST_PORT);
  msnprintf(entry, sizeof(entry), "%s:%d:%s", host, TEST_PORT, SHUFFLE_ADDRS);

  resolve = curl_slist_append(NULL, entry);
  if(!resolve) {
    fprintf(stderr, "curl_slist_append() failed\n");
    return CURLE_OUT_OF_MEMORY;
  }

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    res = CURLE_OUT_OF_MEMORY;
    goto cleanup;
  }

  res = curl_easy_setopt(curl, CURLOPT_URL, url);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_RESOLVE, resolve);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_DNS_SHUFFLE_ADDRESSES, 1L);
  if(res != CURLE_OK)
    goto cleanup;
  res = curl_easy_setopt(curl, CURLOPT_DNS_CACHE_TIMEOUT, 0L);
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
  res = curl_easy_setopt(curl, CURLOPT_OPENSOCKETDATA, &trace);
  if(res != CURLE_OK)
    goto cleanup;

  res = curl_easy_perform(curl);

  if(!trace.saw_ipv4) {
    fprintf(stderr, "did not observe the first connection address\n");
    res = CURLE_BAD_FUNCTION_ARGUMENT;
    goto cleanup;
  }

  msnprintf(primary_ip, primary_ip_size, "%s", trace.first_ip);
  res = CURLE_OK;

cleanup:
  curl_easy_cleanup(curl);
  curl_slist_free_all(resolve);
  return res;
}

int test(char *URL)
{
  CURLcode res = TEST_ERR_MAJOR_BAD;
  char current_ip[64];
  int attempt;
  int reordered = 0;
  (void)URL;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  for(attempt = 0; attempt < NUM_ATTEMPTS; attempt++) {
    current_ip[0] = '\0';
    res = resolve_once(attempt, current_ip, sizeof(current_ip));
    if(res)
      goto test_cleanup;

    if(strcmp(current_ip, "127.0.0.1")) {
      reordered = 1;
      break;
    }
  }

  if(!reordered) {
    fprintf(stderr,
            "DNS shuffle never changed the first connection address\n");
    res = TEST_ERR_MAJOR_BAD;
  }

test_cleanup:
  curl_global_cleanup();
  return (int)res;
}

#else
int test(char *URL)
{
  (void)URL;
  return 0;
}
#endif
