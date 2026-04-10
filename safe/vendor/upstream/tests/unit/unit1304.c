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
#include "curlcheck.h"
#include "netrc.h"
#include "memdebug.h" /* LAST include file */

#ifndef CURL_DISABLE_NETRC

static char *login;
static char *password;

static CURLcode unit_setup(void)
{
  password = NULL;
  login = NULL;
  return CURLE_OK;
}

static void unit_stop(void)
{
  Curl_safefree(password);
  Curl_safefree(login);
}

UNITTEST_START
  int result;

  /*
   * Test a non existent host in our netrc file.
   */
  result = Curl_parsenetrc("test.example.com", &login, &password, arg);
  fail_unless(result == 1, "Host not found should return 1");
  abort_unless(password == NULL, "password did not return NULL!");
  abort_unless(login == NULL, "user did not return NULL!");

  /*
   * Test a non existent login in our netrc file.
   */
  login = (char *)"me";
  result = Curl_parsenetrc("example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password == NULL, "password is not NULL!");

  /*
   * Test a non existent login and host in our netrc file.
   */
  login = (char *)"me";
  result = Curl_parsenetrc("test.example.com", &login, &password, arg);
  fail_unless(result == 1, "Host not found should return 1");
  abort_unless(password == NULL, "password is not NULL!");

  /*
   * Test a non existent login (substring of an existing one) in our
   * netrc file.
   */
  login = (char *)"admi";
  result = Curl_parsenetrc("example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password == NULL, "password is not NULL!");

  /*
   * Test a non existent login (superstring of an existing one)
   * in our netrc file.
   */
  login = (char *)"adminn";
  result = Curl_parsenetrc("example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password == NULL, "password is not NULL!");

  /*
   * Test for the first existing host in our netrc file
   * with login[0] = 0.
   */
  login = NULL;
  result = Curl_parsenetrc("example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password != NULL, "returned NULL!");
  fail_unless(strncmp(password, "passwd", 6) == 0,
              "password should be 'passwd'");
  abort_unless(login != NULL, "returned NULL!");
  fail_unless(strncmp(login, "admin", 5) == 0, "login should be 'admin'");

  /*
   * Test for the first existing host in our netrc file
   * with login[0] != 0.
   */
  free(password);
  free(login);
  password = NULL;
  login = NULL;
  result = Curl_parsenetrc("example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password != NULL, "returned NULL!");
  fail_unless(strncmp(password, "passwd", 6) == 0,
              "password should be 'passwd'");
  abort_unless(login != NULL, "returned NULL!");
  fail_unless(strncmp(login, "admin", 5) == 0, "login should be 'admin'");

  /*
   * Test for the second existing host in our netrc file
   * with login[0] = 0.
   */
  free(password);
  password = NULL;
  free(login);
  login = NULL;
  result = Curl_parsenetrc("curl.example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password != NULL, "returned NULL!");
  fail_unless(strncmp(password, "none", 4) == 0,
              "password should be 'none'");
  abort_unless(login != NULL, "returned NULL!");
  fail_unless(strncmp(login, "none", 4) == 0, "login should be 'none'");

  /*
   * Test for the second existing host in our netrc file
   * with login[0] != 0.
   */
  free(password);
  free(login);
  password = NULL;
  login = NULL;
  result = Curl_parsenetrc("curl.example.com", &login, &password, arg);
  fail_unless(result == 0, "Host should have been found");
  abort_unless(password != NULL, "returned NULL!");
  fail_unless(strncmp(password, "none", 4) == 0,
              "password should be 'none'");
  abort_unless(login != NULL, "returned NULL!");
  fail_unless(strncmp(login, "none", 4) == 0, "login should be 'none'");

UNITTEST_STOP

#else
static CURLcode unit_setup(void)
{
  return CURLE_OK;
}
static void unit_stop(void)
{
}
UNITTEST_START
UNITTEST_STOP

#endif
