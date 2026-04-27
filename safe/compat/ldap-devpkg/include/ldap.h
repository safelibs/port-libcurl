#ifndef PORT_LIBCURL_LDAP_DEVPKG_LDAP_H
#define PORT_LIBCURL_LDAP_DEVPKG_LDAP_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef size_t ber_len_t;

struct berval {
    ber_len_t bv_len;
    char *bv_val;
};

typedef struct ldap LDAP;
typedef struct ldapcontrol LDAPControl;

typedef struct ldapmod {
    int mod_op;
    char *mod_type;
    union {
        char **modv_strvals;
        struct berval **modv_bvals;
    } mod_vals;
} LDAPMod;

#define mod_values mod_vals.modv_strvals
#define mod_bvalues mod_vals.modv_bvals

typedef struct ldapurldesc {
    char *lud_scheme;
    char *lud_host;
    int lud_port;
    char *lud_dn;
    char **lud_attrs;
    int lud_scope;
    char *lud_filter;
    char **lud_exts;
    int lud_crit_exts;
} LDAPURLDesc;

#define LDAP_VERSION3 3

#define LDAP_OPT_PROTOCOL_VERSION 0x0011
#define LDAP_OPT_DEBUG_LEVEL 0x5001

#define LDAP_MOD_OP 0x0007
#define LDAP_MOD_ADD 0x0000
#define LDAP_MOD_DELETE 0x0001
#define LDAP_MOD_REPLACE 0x0002
#define LDAP_MOD_BVALUES 0x0080

#define LDAP_SCOPE_ONELEVEL 1

#define LDAP_SASL_SIMPLE ((char *)0)

int ldap_set_option(LDAP *ld, int option, const void *invalue);
int ldap_initialize(LDAP **ldp, const char *uri);
int ldap_connect(LDAP *ld);
int ldap_unbind_ext(LDAP *ld, LDAPControl **serverctrls, LDAPControl **clientctrls);
int ldap_sasl_bind_s(
    LDAP *ld,
    const char *dn,
    const char *mechanism,
    const struct berval *cred,
    LDAPControl **serverctrls,
    LDAPControl **clientctrls,
    struct berval **servercredp
);
int ldap_add_ext_s(
    LDAP *ld,
    const char *dn,
    LDAPMod **attrs,
    LDAPControl **serverctrls,
    LDAPControl **clientctrls
);
char *ldap_url_desc2str(LDAPURLDesc *ludp);
void ldap_memfree(void *ptr);
const char *ldap_err2string(int err);

#ifdef __cplusplus
}
#endif

#endif
