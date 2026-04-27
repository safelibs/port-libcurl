#ifndef PORT_LIBCURL_LDAP_DEVPKG_LDAP_UTF8_H
#define PORT_LIBCURL_LDAP_DEVPKG_LDAP_UTF8_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

int ldap_x_utf8s_to_mbs(char *dest, const char *src, size_t destlen, void *ctx);

#ifdef __cplusplus
}
#endif

#endif
