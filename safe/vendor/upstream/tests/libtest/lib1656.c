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

static const char expected_expire[] = "2059-02-15 14:43:38 GMT";

static size_t wrfu(void *ptr, size_t size, size_t nmemb, void *stream)
{
  (void)stream;
  (void)ptr;
  return size * nmemb;
}

static const char *find_certinfo_value(struct curl_certinfo *certinfo,
                                       const char *label)
{
  int cert;
  size_t labellen = strlen(label);

  if(!certinfo)
    return NULL;

  for(cert = 0; cert < certinfo->num_of_certs; cert++) {
    struct curl_slist *slist = certinfo->certinfo[cert];

    for(; slist; slist = slist->next) {
      if(!strncmp(slist->data, label, labellen))
        return slist->data + labellen;
    }
  }

  return NULL;
}

int test(char *URL)
{
  CURL *curl;
  CURLcode res = CURLE_OK;
  struct curl_certinfo *certinfo = NULL;
  const char *expire;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    curl_global_cleanup();
    return TEST_ERR_MAJOR_BAD;
  }

  test_setopt(curl, CURLOPT_URL, URL);
  test_setopt(curl, CURLOPT_CERTINFO, 1L);
  test_setopt(curl, CURLOPT_WRITEFUNCTION, wrfu);
  test_setopt(curl, CURLOPT_SSL_VERIFYPEER, 0L);
  test_setopt(curl, CURLOPT_SSL_VERIFYHOST, 0L);

  res = curl_easy_perform(curl);
  if(res && res != CURLE_GOT_NOTHING) {
    fprintf(stderr, "curl_easy_perform() failed: %d\n", res);
    goto test_cleanup;
  }

  res = curl_easy_getinfo(curl, CURLINFO_CERTINFO, &certinfo);
  if(res) {
    fprintf(stderr, "curl_easy_getinfo(CURLINFO_CERTINFO) failed: %d\n", res);
    goto test_cleanup;
  }

  expire = find_certinfo_value(certinfo, "Expire date:");
  if(!expire) {
    fprintf(stderr, "missing certificate expiration field\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  if(strcmp(expire, expected_expire)) {
    fprintf(stderr, "expected expiration '%s', got '%s'\n",
            expected_expire, expire);
    res = TEST_ERR_FAILURE;
  }

test_cleanup:
  curl_easy_cleanup(curl);
  curl_global_cleanup();

  return (int)res;
}
