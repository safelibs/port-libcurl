#ifndef PORT_LIBCURL_LDAP_DEVPKG_LDIF_H
#define PORT_LIBCURL_LDAP_DEVPKG_LDIF_H

#include "ldap.h"

#ifdef __cplusplus
extern "C" {
#endif

char *ldif_getline(char **next);
int ldif_parse_line2(char *line, struct berval *type, struct berval *value, int *freeval);

#ifdef __cplusplus
}
#endif

#endif
