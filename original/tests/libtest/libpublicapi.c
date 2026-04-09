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

#include "memdebug.h"

#define CHECK(cond, msg) do {                                            \
    if(!(cond)) {                                                        \
      fprintf(stderr, "%s:%d %s\n", __FILE__, __LINE__, (msg));          \
      res = TEST_ERR_MAJOR_BAD;                                          \
      goto test_cleanup;                                                 \
    }                                                                    \
  } while(0)

int test(char *URL)
{
  CURLcode res = CURLE_OK;
  CURLcode optres;
  CURL *easy = NULL;
  CURL *dup = NULL;
  CURLM *multi = NULL;
  CURLSH *share = NULL;
  CURLU *url = NULL;
  CURLU *urlcopy = NULL;
  curl_mime *mime = NULL;
  curl_mimepart *part = NULL;
  struct curl_slist *resolve = NULL;
  struct curl_slist *headers = NULL;
  char *escaped = NULL;
  char *unescaped = NULL;
  char *urlstr = NULL;
  char *host = NULL;
  int outlen = 0;
  char *private_data = NULL;

  global_init(CURL_GLOBAL_ALL);

  easy_init(easy);
  easy_setopt(easy, CURLOPT_URL, URL);
  easy_setopt(easy, CURLOPT_PRIVATE, (void *)"public-api-smoke");
  easy_setopt(easy, CURLOPT_NOSIGNAL, 1L);
  easy_setopt(easy, CURLOPT_TIMEOUT_MS, 1000L);
  easy_setopt(easy, CURLOPT_CONNECTTIMEOUT_MS, 1000L);
  easy_setopt(easy, CURLOPT_PROXY, "");
  easy_setopt(easy, CURLOPT_NOPROXY, "localhost,127.0.0.1");
  easy_setopt(easy, CURLOPT_DNS_SHUFFLE_ADDRESSES, 1L);

  optres = curl_easy_setopt(easy, CURLOPT_DOH_URL,
                            "https://example.invalid/dns-query");
  if(optres != CURLE_OK &&
     optres != CURLE_UNKNOWN_OPTION &&
     optres != CURLE_NOT_BUILT_IN) {
    CHECK(0, "CURLOPT_DOH_URL unexpectedly failed");
  }

  escaped = curl_easy_escape(easy, "a/b?c=d", 7);
  CHECK(escaped != NULL, "curl_easy_escape() failed");

  unescaped = curl_easy_unescape(easy, escaped, 0, &outlen);
  CHECK(unescaped != NULL, "curl_easy_unescape() failed");
  CHECK(outlen == 7, "curl_easy_unescape() returned wrong length");
  CHECK(!memcmp(unescaped, "a/b?c=d", 7), "curl_easy_unescape() mismatch");

  CHECK(curl_getdate("Sun, 06 Nov 1994 08:49:37 GMT", NULL) != -1,
        "curl_getdate() failed");

  resolve = curl_slist_append(resolve, "localhost:80:127.0.0.1");
  CHECK(resolve != NULL, "curl_slist_append(resolve) failed");
  headers = curl_slist_append(headers, "X-Test: public-api");
  CHECK(headers != NULL, "curl_slist_append(headers) failed");
  easy_setopt(easy, CURLOPT_RESOLVE, resolve);
  easy_setopt(easy, CURLOPT_HTTPHEADER, headers);

  share = curl_share_init();
  CHECK(share != NULL, "curl_share_init() failed");
  CHECK(curl_share_setopt(share, CURLSHOPT_SHARE,
                          CURL_LOCK_DATA_COOKIE) == CURLSHE_OK,
        "curl_share_setopt() failed");
  easy_setopt(easy, CURLOPT_SHARE, share);

  url = curl_url();
  CHECK(url != NULL, "curl_url() failed");
  CHECK(curl_url_set(url, CURLUPART_URL,
                     "https://user:pass@example.com:8443/base/path?x=1#frag",
                     0) == CURLUE_OK,
        "curl_url_set() failed");
  CHECK(curl_url_get(url, CURLUPART_HOST, &host, 0) == CURLUE_OK,
        "curl_url_get(host) failed");
  CHECK(!strcmp(host, "example.com"), "curl_url_get(host) mismatch");
  curl_free(host);
  host = NULL;

  urlcopy = curl_url_dup(url);
  CHECK(urlcopy != NULL, "curl_url_dup() failed");
  CHECK(curl_url_get(urlcopy, CURLUPART_URL, &urlstr, 0) == CURLUE_OK,
        "curl_url_get(url) failed");
  CHECK(urlstr != NULL && strncmp(urlstr, "https://", 8) == 0,
        "curl_url_get(url) returned bad data");
  curl_free(urlstr);
  urlstr = NULL;

  mime = curl_mime_init(easy);
  CHECK(mime != NULL, "curl_mime_init() failed");
  part = curl_mime_addpart(mime);
  CHECK(part != NULL, "curl_mime_addpart() failed");
  CHECK(curl_mime_name(part, "field") == CURLE_OK,
        "curl_mime_name() failed");
  CHECK(curl_mime_data(part, "value", CURL_ZERO_TERMINATED) == CURLE_OK,
        "curl_mime_data() failed");
  easy_setopt(easy, CURLOPT_MIMEPOST, mime);

  CHECK(curl_easy_getinfo(easy, CURLINFO_PRIVATE, &private_data) == CURLE_OK,
        "curl_easy_getinfo(CURLINFO_PRIVATE) failed");
  CHECK(private_data && !strcmp(private_data, "public-api-smoke"),
        "CURLINFO_PRIVATE mismatch");

  dup = curl_easy_duphandle(easy);
  CHECK(dup != NULL, "curl_easy_duphandle() failed");

  multi_init(multi);
  multi_add_handle(multi, dup);
  curl_multi_remove_handle(multi, dup);
  curl_multi_cleanup(multi);
  multi = NULL;

  curl_easy_reset(dup);
  CHECK(curl_easy_setopt(dup, CURLOPT_URL, URL) == CURLE_OK,
        "curl_easy_reset() left handle unusable");

test_cleanup:
  curl_easy_cleanup(dup);
  if(easy)
    curl_easy_setopt(easy, CURLOPT_MIMEPOST, NULL);
  curl_easy_cleanup(easy);
  curl_mime_free(mime);
  curl_url_cleanup(urlcopy);
  curl_url_cleanup(url);
  curl_share_cleanup(share);
  curl_slist_free_all(headers);
  curl_slist_free_all(resolve);
  curl_free(urlstr);
  curl_free(host);
  curl_free(unescaped);
  curl_free(escaped);
  curl_global_cleanup();
  return (int)res;
}
