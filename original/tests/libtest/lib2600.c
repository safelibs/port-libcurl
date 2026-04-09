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
#ifdef HAVE_NETINET_IN6_H
#include <netinet/in6.h>
#endif
#ifdef HAVE_SYS_SOCKET_H
#include <sys/socket.h>
#endif
#ifdef HAVE_ARPA_INET_H
#include <arpa/inet.h>
#endif
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#endif

#include "testutil.h"
#include "memdebug.h"

#define CONNECT_TIMEOUT_MS      4000L
#define HAPPY_EYEBALLS_MS        200L
#define RELEASE_DELAY_MS         300L
#define TIMING_MIN_MS            150L
#define TIMING_MAX_MS           5000L
#define TEST_ABORT_TIMEOUT_MS  10000L
#define SLOW_BACKLOG              1
#define FAST_BACKLOG              4
#define NUM_FILLERS               2

struct listener_v4 {
  curl_socket_t sock;
  char ip[16];
  unsigned short port;
};

#ifdef ENABLE_IPV6
struct listener_v6 {
  curl_socket_t sock;
  char ip[46];
  unsigned short port;
};
#endif

struct delayed_release {
  curl_socket_t listener;
  curl_socket_t fillers[NUM_FILLERS];
  bool released;
};

static size_t discard_data(char *ptr, size_t size, size_t nmemb, void *userdata)
{
  (void)ptr;
  (void)userdata;
  return size * nmemb;
}

static int set_socket_opt(curl_socket_t sock, int level, int optname, int value)
{
  return setsockopt(sock, level, optname, (void *)&value, sizeof(value));
}

static void close_socket_if_needed(curl_socket_t *sock)
{
  if(*sock != CURL_SOCKET_BAD) {
    sclose(*sock);
    *sock = CURL_SOCKET_BAD;
  }
}

static int open_listener_v4(struct listener_v4 *listener,
                            const char *ip,
                            unsigned short port,
                            int backlog)
{
  struct sockaddr_in addr;
  struct sockaddr_in bound;
  socklen_t bound_len = sizeof(bound);

  memset(listener, 0, sizeof(*listener));
  listener->sock = CURL_SOCKET_BAD;

  listener->sock = socket(AF_INET, SOCK_STREAM, 0);
  if(listener->sock == CURL_SOCKET_BAD) {
    fprintf(stderr, "socket(AF_INET) failed for %s\n", ip);
    return TEST_ERR_MAJOR_BAD;
  }

  if(set_socket_opt(listener->sock, SOL_SOCKET, SO_REUSEADDR, 1) < 0) {
    fprintf(stderr, "setsockopt(SO_REUSEADDR) failed for %s\n", ip);
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  if(inet_pton(AF_INET, ip, &addr.sin_addr) != 1) {
    fprintf(stderr, "inet_pton(AF_INET, %s) failed\n", ip);
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  if(bind(listener->sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    fprintf(stderr, "bind(%s:%u) failed\n", ip, (unsigned int)port);
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  if(listen(listener->sock, backlog) < 0) {
    fprintf(stderr, "listen(%s:%u) failed\n", ip, (unsigned int)port);
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  if(getsockname(listener->sock, (struct sockaddr *)&bound, &bound_len) < 0) {
    fprintf(stderr, "getsockname(%s) failed\n", ip);
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  listener->port = ntohs(bound.sin_port);
  msnprintf(listener->ip, sizeof(listener->ip), "%s", ip);
  return 0;
}

static int open_client_v4_socket(const char *ip, unsigned short port)
{
  curl_socket_t sock = CURL_SOCKET_BAD;
  struct sockaddr_in addr;

  sock = socket(AF_INET, SOCK_STREAM, 0);
  if(sock == CURL_SOCKET_BAD) {
    fprintf(stderr, "socket(AF_INET) failed for filler %s\n", ip);
    return CURL_SOCKET_BAD;
  }

  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_port = htons(port);
  if(inet_pton(AF_INET, ip, &addr.sin_addr) != 1) {
    fprintf(stderr, "inet_pton(AF_INET, %s) failed for filler\n", ip);
    sclose(sock);
    return CURL_SOCKET_BAD;
  }

  if(connect(sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    fprintf(stderr, "connect(%s:%u) failed for filler\n", ip,
            (unsigned int)port);
    sclose(sock);
    return CURL_SOCKET_BAD;
  }

  return sock;
}

static int saturate_v4_listener(struct delayed_release *release,
                                const struct listener_v4 *listener)
{
  size_t i;

  memset(release, 0, sizeof(*release));
  release->listener = listener->sock;
  for(i = 0; i < NUM_FILLERS; ++i)
    release->fillers[i] = CURL_SOCKET_BAD;

  for(i = 0; i < NUM_FILLERS; ++i) {
    release->fillers[i] = open_client_v4_socket(listener->ip, listener->port);
    if(release->fillers[i] == CURL_SOCKET_BAD)
      return TEST_ERR_MAJOR_BAD;
  }
  return 0;
}

#ifdef ENABLE_IPV6
static int open_listener_v6(struct listener_v6 *listener,
                            const char *ip,
                            unsigned short port,
                            int backlog)
{
  struct sockaddr_in6 addr;
  struct sockaddr_in6 bound;
  socklen_t bound_len = sizeof(bound);

  memset(listener, 0, sizeof(*listener));
  listener->sock = CURL_SOCKET_BAD;

  listener->sock = socket(AF_INET6, SOCK_STREAM, 0);
  if(listener->sock == CURL_SOCKET_BAD)
    return TEST_ERR_FAILURE;

  if(set_socket_opt(listener->sock, SOL_SOCKET, SO_REUSEADDR, 1) < 0) {
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }
#ifdef IPV6_V6ONLY
  if(set_socket_opt(listener->sock, IPPROTO_IPV6, IPV6_V6ONLY, 1) < 0) {
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }
#endif

  memset(&addr, 0, sizeof(addr));
  addr.sin6_family = AF_INET6;
  addr.sin6_port = htons(port);
  if(inet_pton(AF_INET6, ip, &addr.sin6_addr) != 1) {
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  if(bind(listener->sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_FAILURE;
  }

  if(listen(listener->sock, backlog) < 0) {
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  if(getsockname(listener->sock, (struct sockaddr *)&bound, &bound_len) < 0) {
    close_socket_if_needed(&listener->sock);
    return TEST_ERR_MAJOR_BAD;
  }

  listener->port = ntohs(bound.sin6_port);
  msnprintf(listener->ip, sizeof(listener->ip), "%s", ip);
  return 0;
}

static int open_client_v6_socket(const char *ip, unsigned short port)
{
  curl_socket_t sock = CURL_SOCKET_BAD;
  struct sockaddr_in6 addr;

  sock = socket(AF_INET6, SOCK_STREAM, 0);
  if(sock == CURL_SOCKET_BAD)
    return CURL_SOCKET_BAD;

  memset(&addr, 0, sizeof(addr));
  addr.sin6_family = AF_INET6;
  addr.sin6_port = htons(port);
  if(inet_pton(AF_INET6, ip, &addr.sin6_addr) != 1) {
    sclose(sock);
    return CURL_SOCKET_BAD;
  }

  if(connect(sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
    sclose(sock);
    return CURL_SOCKET_BAD;
  }

  return sock;
}

static int saturate_v6_listener(struct delayed_release *release,
                                const struct listener_v6 *listener)
{
  size_t i;

  memset(release, 0, sizeof(*release));
  release->listener = listener->sock;
  for(i = 0; i < NUM_FILLERS; ++i)
    release->fillers[i] = CURL_SOCKET_BAD;

  for(i = 0; i < NUM_FILLERS; ++i) {
    release->fillers[i] = open_client_v6_socket(listener->ip, listener->port);
    if(release->fillers[i] == CURL_SOCKET_BAD)
      return TEST_ERR_FAILURE;
  }
  return 0;
}
#endif

static void release_slow_path(struct delayed_release *release)
{
  size_t i;

  if(release->released)
    return;

  close_socket_if_needed(&release->listener);
  for(i = 0; i < NUM_FILLERS; ++i)
    close_socket_if_needed(&release->fillers[i]);
  release->released = TRUE;
}

static void cleanup_release(struct delayed_release *release)
{
  release_slow_path(release);
}

static int accept_and_close(curl_socket_t listener)
{
  curl_socket_t accepted = CURL_SOCKET_BAD;

  accepted = accept(listener, NULL, NULL);
  if(accepted == CURL_SOCKET_BAD) {
    fprintf(stderr, "accept() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  sclose(accepted);
  return 0;
}

static int run_connect_case(const char *name,
                            const char *url,
                            const char *resolve_entry,
                            long he_timeout_ms,
                            struct delayed_release *release,
                            const char *expected_ip)
{
  CURL *easy = NULL;
  CURLM *multi = NULL;
  struct curl_slist *resolve = NULL;
  CURLMsg *msg;
  int msgs_left = 0;
  int still_running = 0;
  int res = 0;
  CURLcode easy_res;
  bool got_done = FALSE;
  bool added = FALSE;
  char *primary_ip = NULL;
  curl_off_t connect_time_us = 0;
  struct timeval started = tutil_tvnow();

  easy = curl_easy_init();
  if(!easy) {
    fprintf(stderr, "%s: curl_easy_init() failed\n", name);
    return TEST_ERR_MAJOR_BAD;
  }

  multi = curl_multi_init();
  if(!multi) {
    fprintf(stderr, "%s: curl_multi_init() failed\n", name);
    curl_easy_cleanup(easy);
    return TEST_ERR_MAJOR_BAD;
  }

  resolve = curl_slist_append(NULL, resolve_entry);
  if(!resolve) {
    fprintf(stderr, "%s: curl_slist_append() failed\n", name);
    res = TEST_ERR_MAJOR_BAD;
    goto cleanup;
  }

  easy_res = curl_easy_setopt(easy, CURLOPT_URL, url);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_URL failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_RESOLVE, resolve);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_RESOLVE failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_CONNECT_ONLY, 1L);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_CONNECT_ONLY failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_FRESH_CONNECT, 1L);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_FRESH_CONNECT failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_NOSIGNAL, 1L);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_NOSIGNAL failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_TIMEOUT_MS, CONNECT_TIMEOUT_MS);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_TIMEOUT_MS failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_CONNECTTIMEOUT_MS,
                              CONNECT_TIMEOUT_MS);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_CONNECTTIMEOUT_MS failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_HAPPY_EYEBALLS_TIMEOUT_MS,
                              he_timeout_ms);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_HAPPY_EYEBALLS_TIMEOUT_MS failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_PROXY, "");
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_PROXY failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_NOPROXY, "*");
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_NOPROXY failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }
  easy_res = curl_easy_setopt(easy, CURLOPT_WRITEFUNCTION, discard_data);
  if(easy_res != CURLE_OK) {
    fprintf(stderr, "%s: CURLOPT_WRITEFUNCTION failed: %s\n",
            name, curl_easy_strerror(easy_res));
    res = (int)easy_res;
    goto cleanup;
  }

  {
    CURLMcode mres = curl_multi_add_handle(multi, easy);
    if(mres != CURLM_OK) {
      fprintf(stderr, "%s: curl_multi_add_handle() failed: %s\n",
              name, curl_multi_strerror(mres));
      res = TEST_ERR_MULTI;
      goto cleanup;
    }
    added = TRUE;
  }

  do {
    CURLMcode mres = curl_multi_perform(multi, &still_running);
    if(mres != CURLM_OK) {
      fprintf(stderr, "%s: curl_multi_perform() failed: %s\n",
              name, curl_multi_strerror(mres));
      res = TEST_ERR_MULTI;
      goto cleanup;
    }

    if(release && !release->released &&
       (tutil_tvdiff(tutil_tvnow(), started) >= RELEASE_DELAY_MS))
      release_slow_path(release);

    while((msg = curl_multi_info_read(multi, &msgs_left))) {
      if(msg->msg == CURLMSG_DONE && msg->easy_handle == easy) {
        res = msg->data.result;
        got_done = TRUE;
      }
    }

    if(got_done && !still_running)
      break;

    if(tutil_tvdiff(tutil_tvnow(), started) > TEST_ABORT_TIMEOUT_MS) {
      fprintf(stderr, "%s: timed out waiting for completion\n", name);
      res = TEST_ERR_RUNS_FOREVER;
      goto cleanup;
    }

    if(still_running || !got_done) {
      int numfds = 0;
      CURLMcode mres2 = curl_multi_poll(multi, NULL, 0, 100, &numfds);
      if(mres2 != CURLM_OK) {
        fprintf(stderr, "%s: curl_multi_poll() failed: %s\n",
                name, curl_multi_strerror(mres2));
        res = TEST_ERR_MULTI;
        goto cleanup;
      }
    }
  } while(!got_done);

  if(res) {
    fprintf(stderr, "%s: transfer failed with %d\n", name, res);
    goto cleanup;
  }

  if(curl_easy_getinfo(easy, CURLINFO_PRIMARY_IP, &primary_ip) != CURLE_OK ||
     !primary_ip) {
    fprintf(stderr, "%s: CURLINFO_PRIMARY_IP failed\n", name);
    res = TEST_ERR_MAJOR_BAD;
    goto cleanup;
  }

  if(strcmp(primary_ip, expected_ip)) {
    fprintf(stderr, "%s: expected primary IP %s, got %s\n",
            name, expected_ip, primary_ip);
    res = TEST_ERR_MAJOR_BAD;
    goto cleanup;
  }

  if(curl_easy_getinfo(easy, CURLINFO_CONNECT_TIME_T, &connect_time_us)
     != CURLE_OK) {
    fprintf(stderr, "%s: CURLINFO_CONNECT_TIME_T failed\n", name);
    res = TEST_ERR_MAJOR_BAD;
    goto cleanup;
  }

  if(connect_time_us < (TIMING_MIN_MS * 1000)) {
    fprintf(stderr, "%s: connect time too short: %ld us\n",
            name, (long)connect_time_us);
    res = TEST_ERR_MAJOR_BAD;
    goto cleanup;
  }

  if(connect_time_us > (TIMING_MAX_MS * 1000)) {
    fprintf(stderr, "%s: connect time too long: %ld us\n",
            name, (long)connect_time_us);
    res = TEST_ERR_MAJOR_BAD;
    goto cleanup;
  }

cleanup:
  if(multi && easy && added)
    curl_multi_remove_handle(multi, easy);
  curl_slist_free_all(resolve);
  curl_multi_cleanup(multi);
  curl_easy_cleanup(easy);
  return res;
}

static int run_ipv4_retry_case(void)
{
  struct listener_v4 slow;
  struct listener_v4 fast;
  struct delayed_release release;
  char url[128];
  char resolve_entry[128];
  int res = 0;

  res = open_listener_v4(&slow, "127.0.0.1", 0, SLOW_BACKLOG);
  if(res)
    return res;

  res = open_listener_v4(&fast, "127.0.0.2", slow.port, FAST_BACKLOG);
  if(res) {
    close_socket_if_needed(&slow.sock);
    return res;
  }

  res = saturate_v4_listener(&release, &slow);
  if(res)
    goto cleanup;

  msnprintf(url, sizeof(url), "http://timing-v4.example:%u/", slow.port);
  msnprintf(resolve_entry, sizeof(resolve_entry),
            "timing-v4.example:%u:127.0.0.1,127.0.0.2", slow.port);

  res = run_connect_case("ipv4-retry", url, resolve_entry,
                         HAPPY_EYEBALLS_MS, &release, "127.0.0.2");
  if(!res)
    res = accept_and_close(fast.sock);

cleanup:
  cleanup_release(&release);
  slow.sock = CURL_SOCKET_BAD;
  close_socket_if_needed(&fast.sock);
  return res;
}

#ifdef ENABLE_IPV6
static int run_happy_eyeballs_case(void)
{
  struct listener_v4 fast;
  struct listener_v6 slow;
  struct delayed_release release;
  char url[128];
  char resolve_entry[160];
  int res = 0;

  res = open_listener_v4(&fast, "127.0.0.1", 0, FAST_BACKLOG);
  if(res)
    return res;

  res = open_listener_v6(&slow, "::1", fast.port, SLOW_BACKLOG);
  if(res == TEST_ERR_FAILURE) {
    fprintf(stderr, "IPv6 loopback unavailable, skipping Happy Eyeballs case\n");
    close_socket_if_needed(&fast.sock);
    return 0;
  }
  if(res) {
    close_socket_if_needed(&fast.sock);
    return res;
  }

  res = saturate_v6_listener(&release, &slow);
  if(res == TEST_ERR_FAILURE) {
    fprintf(stderr, "IPv6 backlog setup unavailable, skipping Happy Eyeballs case\n");
    close_socket_if_needed(&slow.sock);
    close_socket_if_needed(&fast.sock);
    return 0;
  }
  if(res)
    goto cleanup;

  msnprintf(url, sizeof(url), "http://timing-he.example:%u/", fast.port);
  msnprintf(resolve_entry, sizeof(resolve_entry),
            "timing-he.example:%u:[::1],127.0.0.1", fast.port);

  res = run_connect_case("happy-eyeballs", url, resolve_entry,
                         HAPPY_EYEBALLS_MS, &release, "127.0.0.1");
  if(!res)
    res = accept_and_close(fast.sock);

cleanup:
  cleanup_release(&release);
  slow.sock = CURL_SOCKET_BAD;
  close_socket_if_needed(&fast.sock);
  return res;
}
#endif

int test(char *URL)
{
  int res;

  (void)URL;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  res = run_ipv4_retry_case();
  if(res)
    goto cleanup;

#ifdef ENABLE_IPV6
  res = run_happy_eyeballs_case();
  if(res)
    goto cleanup;
#endif

cleanup:
  curl_global_cleanup();
  return res;
}
