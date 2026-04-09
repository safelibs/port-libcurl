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

static int perform_auth_request(const char *url, long auth, const char *userpwd)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;

  curl = curl_easy_init();
  if(!curl) {
    fprintf(stderr, "curl_easy_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  test_setopt(curl, CURLOPT_URL, url);
  test_setopt(curl, CURLOPT_HEADER, 1L);
  test_setopt(curl, CURLOPT_HTTPAUTH, auth);
  test_setopt(curl, CURLOPT_USERPWD, userpwd);
  test_setopt(curl, CURLOPT_PROXY, "");
  test_setopt(curl, CURLOPT_NOPROXY, "*");
  test_setopt(curl, CURLOPT_NOSIGNAL, 1L);

  res = curl_easy_perform(curl);

test_cleanup:
  curl_easy_cleanup(curl);
  return (int)res;
}

int test(char *URL)
{
  int res = 0;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  res = perform_auth_request(URL, CURLAUTH_BASIC, "i:i");
  if(res)
    goto test_cleanup;

  res = perform_auth_request(URL, CURLAUTH_BASIC, "ii:i");
  if(res)
    goto test_cleanup;

  res = perform_auth_request(URL, CURLAUTH_BASIC, "iii:i");
  if(res)
    goto test_cleanup;

  /* NTLM keeps public base64 decode coverage via the challenge response. */
  res = perform_auth_request(URL, CURLAUTH_NTLM, "testuser:testpass");

test_cleanup:
  curl_global_cleanup();
  return res;
}
