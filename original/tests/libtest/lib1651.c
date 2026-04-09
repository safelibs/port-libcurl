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

static size_t wrfu(void *ptr, size_t size, size_t nmemb, void *stream)
{
  (void)stream;
  (void)ptr;
  return size * nmemb;
}

static const char *find_certinfo_value(struct curl_certinfo *certinfo,
                                       int certnum,
                                       const char *label)
{
  struct curl_slist *slist;
  size_t labellen = strlen(label);

  if(!certinfo || (certnum < 0) || (certnum >= certinfo->num_of_certs))
    return NULL;

  for(slist = certinfo->certinfo[certnum]; slist; slist = slist->next) {
    if(!strncmp(slist->data, label, labellen))
      return slist->data + labellen;
  }

  return NULL;
}

static bool contains_any(const char *value,
                         const char *token1,
                         const char *token2,
                         const char *token3)
{
  return (token1 && strstr(value, token1)) ||
         (token2 && strstr(value, token2)) ||
         (token3 && strstr(value, token3));
}

static CURLcode require_token(const char *field,
                              const char *value,
                              const char *token)
{
  if(!value) {
    fprintf(stderr, "missing certificate field: %s\n", field);
    return TEST_ERR_FAILURE;
  }

  if(!strstr(value, token)) {
    fprintf(stderr, "%s did not contain '%s': %s\n", field, token, value);
    return TEST_ERR_FAILURE;
  }

  return CURLE_OK;
}

int test(char *URL)
{
  CURL *curl = NULL;
  CURLcode res = CURLE_OK;
  struct curl_certinfo *certinfo = NULL;
  const char *subject;
  const char *issuer;
  const char *version;
  const char *signature_algorithm;

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
  if(res && (res != CURLE_GOT_NOTHING)) {
    fprintf(stderr, "curl_easy_perform() failed: %d\n", res);
    goto test_cleanup;
  }

  res = curl_easy_getinfo(curl, CURLINFO_CERTINFO, &certinfo);
  if(res) {
    fprintf(stderr, "curl_easy_getinfo(CURLINFO_CERTINFO) failed: %d\n", res);
    goto test_cleanup;
  }

  if(!certinfo || (certinfo->num_of_certs < 1)) {
    fprintf(stderr, "missing certificate chain data\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  subject = find_certinfo_value(certinfo, 0, "Subject:");
  issuer = find_certinfo_value(certinfo, 0, "Issuer:");
  version = find_certinfo_value(certinfo, 0, "Version:");
  signature_algorithm =
    find_certinfo_value(certinfo, 0, "Signature Algorithm:");

  res = require_token("Subject", subject,
                      "Edel Curl Arctic Illudium Research Cloud");
  if(res)
    goto test_cleanup;

  res = require_token("Subject", subject, "localhost");
  if(res)
    goto test_cleanup;

  res = require_token("Issuer", issuer,
                      "Edel Curl Arctic Illudium Research Cloud");
  if(res)
    goto test_cleanup;

  res = require_token("Issuer", issuer, "Northern Nowhere Trust Anchor");
  if(res)
    goto test_cleanup;

  if(!version) {
    fprintf(stderr, "missing certificate field: Version\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  if(strcmp(version, "2")) {
    fprintf(stderr, "unexpected certificate version: %s\n", version);
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  if(!signature_algorithm) {
    fprintf(stderr, "missing certificate field: Signature Algorithm\n");
    res = TEST_ERR_FAILURE;
    goto test_cleanup;
  }

  if(!contains_any(signature_algorithm, "sha256", "SHA256", "SHA-256") ||
     !contains_any(signature_algorithm, "rsa", "RSA", NULL)) {
    fprintf(stderr, "unexpected signature algorithm: %s\n",
            signature_algorithm);
    res = TEST_ERR_FAILURE;
  }

test_cleanup:
  curl_easy_cleanup(curl);
  curl_global_cleanup();

  return (int)res;
}
