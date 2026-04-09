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
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#endif

#include "memdebug.h"

#define NUM_ATTEMPTS 32

static CURLcode open_listener(curl_socket_t *listener, unsigned short *port)
{
  struct sockaddr_in addr;
  struct sockaddr_in bound;
  socklen_t bound_len = sizeof(bound);

  *listener = socket(AF_INET, SOCK_STREAM, 0);
  if(*listener == CURL_SOCKET_BAD) {
    fprintf(stderr, "socket() failed\n");
    return CURLE_COULDNT_CONNECT;
  }

  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = htonl(INADDR_ANY);
  addr.sin_port = htons(0);

  if(bind(*listener, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    fprintf(stderr, "bind() failed\n");
    return CURLE_COULDNT_CONNECT;
  }

  if(listen(*listener, 8) < 0) {
    fprintf(stderr, "listen() failed\n");
    return CURLE_COULDNT_CONNECT;
  }

  if(getsockname(*listener, (struct sockaddr *)&bound, &bound_len) < 0) {
    fprintf(stderr, "getsockname() failed\n");
    return CURLE_COULDNT_CONNECT;
  }

  *port = ntohs(bound.sin_port);
  return CURLE_OK;
}

static CURLcode connect_once(curl_socket_t listener,
                             unsigned short port,
                             int attempt,
                             char *primary_ip,
                             size_t primary_ip_size)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  struct curl_slist *resolve = NULL;
  curl_socket_t accepted = CURL_SOCKET_BAD;
  char url[128];
  char host[64];
  char entry[128];
  char *ip = NULL;

  msnprintf(host, sizeof(host), "shuffle-%d.example", attempt);
  msnprintf(url, sizeof(url), "http://%s:%u/", host, port);
  msnprintf(entry, sizeof(entry), "%s:%u:127.0.0.1,127.0.0.2", host, port);

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

  if((res = curl_easy_setopt(curl, CURLOPT_URL, url)) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_RESOLVE, resolve)) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_DNS_SHUFFLE_ADDRESSES, 1L))
     != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_DNS_CACHE_TIMEOUT, 0L))
     != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_CONNECT_ONLY, 1L)) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_FRESH_CONNECT, 1L)) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_NOSIGNAL, 1L)) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, 2000L)) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, 2000L))
     != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_PROXY, "")) != CURLE_OK)
    goto cleanup;
  if((res = curl_easy_setopt(curl, CURLOPT_NOPROXY, "*")) != CURLE_OK)
    goto cleanup;

  res = curl_easy_perform(curl);
  if(res)
    goto cleanup;

  accepted = accept(listener, NULL, NULL);
  if(accepted == CURL_SOCKET_BAD) {
    fprintf(stderr, "accept() failed\n");
    res = CURLE_COULDNT_CONNECT;
    goto cleanup;
  }

  res = curl_easy_getinfo(curl, CURLINFO_PRIMARY_IP, &ip);
  if(res || !ip) {
    fprintf(stderr, "CURLINFO_PRIMARY_IP failed\n");
    res = CURLE_BAD_FUNCTION_ARGUMENT;
    goto cleanup;
  }

  msnprintf(primary_ip, primary_ip_size, "%s", ip);

cleanup:
  if(accepted != CURL_SOCKET_BAD)
    sclose(accepted);
  curl_easy_cleanup(curl);
  curl_slist_free_all(resolve);
  return res;
}

int test(char *URL)
{
  CURLcode res = TEST_ERR_MAJOR_BAD;
  curl_socket_t listener = CURL_SOCKET_BAD;
  unsigned short port = 0;
  char first_ip[64] = "";
  char current_ip[64];
  int attempt;
  int reordered = 0;
  (void)URL;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  res = open_listener(&listener, &port);
  if(res)
    goto test_cleanup;

  for(attempt = 0; attempt < NUM_ATTEMPTS; attempt++) {
    current_ip[0] = '\0';
    res = connect_once(listener, port, attempt, current_ip,
                       sizeof(current_ip));
    if(res)
      goto test_cleanup;

    if(!attempt)
      msnprintf(first_ip, sizeof(first_ip), "%s", current_ip);
    else if(strcmp(first_ip, current_ip)) {
      reordered = 1;
      break;
    }
  }

  if(!reordered) {
    fprintf(stderr, "DNS shuffle never changed the selected address\n");
    res = TEST_ERR_MAJOR_BAD;
  }

test_cleanup:
  if(listener != CURL_SOCKET_BAD)
    sclose(listener);
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
