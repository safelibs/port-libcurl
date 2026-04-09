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

int test(char *URL)
{
  CURL *easy = NULL;
  CURLcode res = CURLE_OK;
  int outlen = 0;
  char *ptr;

  (void)URL;

  if(curl_global_init(CURL_GLOBAL_ALL) != CURLE_OK) {
    fprintf(stderr, "curl_global_init() failed\n");
    return TEST_ERR_MAJOR_BAD;
  }

  easy = curl_easy_init();
  if(!easy) {
    fprintf(stderr, "curl_easy_init() failed\n");
    res = TEST_ERR_EASY_INIT;
    goto test_cleanup;
  }

  ptr = curl_easy_escape(easy, "", -1);
  if(ptr) {
    fprintf(stderr, "curl_easy_escape(..., -1) unexpectedly returned data\n");
    curl_free(ptr);
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  outlen = 2017;
  ptr = curl_easy_unescape(easy, "%41%41%41%41", -1, &outlen);
  if(ptr) {
    fprintf(stderr, "curl_easy_unescape(..., -1, ...) unexpectedly returned "
            "data\n");
    curl_free(ptr);
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

test_cleanup:
  curl_easy_cleanup(easy);
  curl_global_cleanup();

  return (int)res;
}
