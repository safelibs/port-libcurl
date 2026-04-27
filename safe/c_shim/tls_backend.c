#include <errno.h>
#include <stdarg.h>
#include <stdint.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#ifdef SAFE_TLS_OPENSSL
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/pem.h>
#include <openssl/sha.h>
#include <openssl/ssl.h>
#include <openssl/x509.h>
#endif

#ifdef SAFE_TLS_GNUTLS
#include <gnutls/abstract.h>
#include <gnutls/crypto.h>
#include <gnutls/gnutls.h>
#include <gnutls/x509.h>
#endif

struct safe_tls_connection;

static void set_error(char *errbuf, size_t errlen, const char *msg)
{
  if(!errbuf || !errlen)
    return;
  if(!msg)
    msg = "unknown TLS error";
  snprintf(errbuf, errlen, "%s", msg);
}

static int load_file_bytes(const char *path, unsigned char **out, size_t *out_len)
{
  FILE *fp;
  long size;
  unsigned char *buf;

  *out = NULL;
  *out_len = 0;
  if(!path || !*path)
    return -1;

  fp = fopen(path, "rb");
  if(!fp)
    return -1;
  if(fseek(fp, 0, SEEK_END)) {
    fclose(fp);
    return -1;
  }
  size = ftell(fp);
  if(size < 0) {
    fclose(fp);
    return -1;
  }
  if(fseek(fp, 0, SEEK_SET)) {
    fclose(fp);
    return -1;
  }
  buf = malloc((size_t)size + 1);
  if(!buf) {
    fclose(fp);
    return -1;
  }
  if(size && fread(buf, 1, (size_t)size, fp) != (size_t)size) {
    fclose(fp);
    free(buf);
    return -1;
  }
  fclose(fp);
  buf[size] = 0;
  *out = buf;
  *out_len = (size_t)size;
  return 0;
}

static size_t base64_encode(const unsigned char *input, size_t input_len,
                            char *output, size_t output_len)
{
  static const char table[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  size_t in = 0;
  size_t out = 0;

  if(output_len == 0)
    return 0;

  while(in < input_len && out + 4 < output_len) {
    uint32_t chunk = (uint32_t)input[in++] << 16;
    int pad = 2;
    if(in <= input_len) {
      if(in < input_len) {
        chunk |= (uint32_t)input[in++] << 8;
        pad = 1;
      }
      if(in < input_len) {
        chunk |= (uint32_t)input[in++];
        pad = 0;
      }
    }
    output[out++] = table[(chunk >> 18) & 0x3f];
    output[out++] = table[(chunk >> 12) & 0x3f];
    output[out++] = (pad >= 2) ? '=' : table[(chunk >> 6) & 0x3f];
    output[out++] = (pad >= 1) ? '=' : table[chunk & 0x3f];
  }

  output[out] = '\0';
  return out;
}

struct certinfo_builder {
  char *buf;
  size_t len;
  size_t cap;
};

static int certinfo_append(struct certinfo_builder *builder,
                           const char *fmt, ...)
{
  va_list ap;
  va_list copy;
  int needed;
  size_t required;
  char *grown;

  va_start(ap, fmt);
  va_copy(copy, ap);
  needed = vsnprintf(NULL, 0, fmt, copy);
  va_end(copy);
  if(needed < 0) {
    va_end(ap);
    return -1;
  }
  required = builder->len + (size_t)needed + 1;
  if(required > builder->cap) {
    size_t next_cap = builder->cap ? builder->cap : 256;
    while(next_cap < required)
      next_cap *= 2;
    grown = realloc(builder->buf, next_cap);
    if(!grown) {
      va_end(ap);
      return -1;
    }
    builder->buf = grown;
    builder->cap = next_cap;
  }
  vsnprintf(builder->buf + builder->len, builder->cap - builder->len, fmt, ap);
  builder->len += (size_t)needed;
  va_end(ap);
  return 0;
}

static unsigned char *certinfo_take(struct certinfo_builder *builder,
                                    size_t *out_len)
{
  unsigned char *out;
  if(out_len)
    *out_len = builder->len;
  out = (unsigned char *)builder->buf;
  builder->buf = NULL;
  builder->len = 0;
  builder->cap = 0;
  return out;
}

static void certinfo_builder_cleanup(struct certinfo_builder *builder)
{
  free(builder->buf);
  builder->buf = NULL;
  builder->len = 0;
  builder->cap = 0;
}

static int append_certinfo_line(struct certinfo_builder *builder,
                                unsigned int certnum,
                                const char *label,
                                const char *value)
{
  return certinfo_append(builder, "%u\t%s: %s\n", certnum, label,
                         value ? value : "");
}

static int safe_tls_map_alpn(const unsigned char *protocol, size_t protocol_len)
{
  if(protocol && protocol_len == 2 && memcmp(protocol, "h2", 2) == 0)
    return 2;
  if(protocol && protocol_len == 8 && memcmp(protocol, "http/1.1", 8) == 0)
    return 1;
  return 0;
}

#ifdef SAFE_TLS_GNUTLS
static int append_certinfo_time(struct certinfo_builder *builder,
                                unsigned int certnum,
                                const char *label,
                                time_t when)
{
  char rendered[64];
  struct tm *tm_buf = gmtime(&when);
  if(!tm_buf)
    return append_certinfo_line(builder, certnum, label, "");
  if(strftime(rendered, sizeof(rendered), "%b %e %H:%M:%S %Y GMT", tm_buf) == 0)
    return append_certinfo_line(builder, certnum, label, "");
  return append_certinfo_line(builder, certnum, label, rendered);
}
#endif

#ifdef SAFE_TLS_OPENSSL

struct safe_tls_connection {
  SSL *ssl;
  int negotiated_alpn;
};

static pthread_once_t openssl_once = PTHREAD_ONCE_INIT;
static SSL_CTX *openssl_verify_ctx = NULL;
static SSL_CTX *openssl_noverify_ctx = NULL;

static void openssl_global_init(void)
{
  OPENSSL_init_ssl(0, NULL);

  openssl_noverify_ctx = SSL_CTX_new(TLS_client_method());
  if(openssl_noverify_ctx) {
    SSL_CTX_set_session_cache_mode(openssl_noverify_ctx, SSL_SESS_CACHE_CLIENT);
    SSL_CTX_set_verify(openssl_noverify_ctx, SSL_VERIFY_NONE, NULL);
  }

  openssl_verify_ctx = SSL_CTX_new(TLS_client_method());
  if(openssl_verify_ctx) {
    SSL_CTX_set_session_cache_mode(openssl_verify_ctx, SSL_SESS_CACHE_CLIENT);
    SSL_CTX_set_verify(openssl_verify_ctx, SSL_VERIFY_PEER, NULL);
    SSL_CTX_set_default_verify_paths(openssl_verify_ctx);
  }
}

static SSL_CTX *openssl_shared_ctx(int verify_peer)
{
  pthread_once(&openssl_once, openssl_global_init);
  return verify_peer ? openssl_verify_ctx : openssl_noverify_ctx;
}

static int openssl_export_session(SSL *ssl,
                                  unsigned char **out_session_data,
                                  size_t *out_session_len)
{
  SSL_SESSION *session = NULL;
  unsigned char *encoded = NULL;
  int encoded_len;

  *out_session_data = NULL;
  *out_session_len = 0;
  if(!ssl)
    return -1;

  session = SSL_get1_session(ssl);
  if(!session)
    return -1;

  encoded_len = i2d_SSL_SESSION(session, NULL);
  if(encoded_len > 0) {
    encoded = malloc((size_t)encoded_len);
    if(encoded) {
      unsigned char *cursor = encoded;
      if(i2d_SSL_SESSION(session, &cursor) == encoded_len) {
        *out_session_data = encoded;
        *out_session_len = (size_t)encoded_len;
      }
      else
        free(encoded);
    }
  }
  SSL_SESSION_free(session);
  return (*out_session_data && *out_session_len) ? 0 : -1;
}

static int openssl_export_pubkey_der(X509 *cert, unsigned char **out, int *out_len)
{
  EVP_PKEY *pkey;
  unsigned char *der = NULL;
  int len;

  *out = NULL;
  *out_len = 0;
  if(!cert)
    return -1;

  pkey = X509_get_pubkey(cert);
  if(!pkey)
    return -1;
  len = i2d_PUBKEY(pkey, NULL);
  if(len <= 0) {
    EVP_PKEY_free(pkey);
    return -1;
  }
  der = malloc((size_t)len);
  if(!der) {
    EVP_PKEY_free(pkey);
    return -1;
  }
  {
    unsigned char *tmp = der;
    if(i2d_PUBKEY(pkey, &tmp) != len) {
      EVP_PKEY_free(pkey);
      free(der);
      return -1;
    }
  }
  EVP_PKEY_free(pkey);
  *out = der;
  *out_len = len;
  return 0;
}

static int openssl_load_pinned_pubkey_der(const char *spec,
                                          unsigned char **out, int *out_len)
{
  unsigned char *file_bytes = NULL;
  size_t file_len = 0;
  BIO *bio = NULL;
  EVP_PKEY *pkey = NULL;
  int len;

  *out = NULL;
  *out_len = 0;
  if(load_file_bytes(spec, &file_bytes, &file_len))
    return -1;

  bio = BIO_new_mem_buf(file_bytes, (int)file_len);
  if(bio)
    pkey = PEM_read_bio_PUBKEY(bio, NULL, NULL, NULL);
  if(!pkey) {
    const unsigned char *tmp = file_bytes;
    pkey = d2i_PUBKEY(NULL, &tmp, (long)file_len);
  }
  if(!pkey) {
    X509 *cert = NULL;
    if(bio) {
      BIO_free(bio);
      bio = BIO_new_mem_buf(file_bytes, (int)file_len);
    }
    if(bio)
      cert = PEM_read_bio_X509(bio, NULL, NULL, NULL);
    if(!cert) {
      const unsigned char *tmp = file_bytes;
      cert = d2i_X509(NULL, &tmp, (long)file_len);
    }
    if(cert) {
      pkey = X509_get_pubkey(cert);
      X509_free(cert);
    }
  }
  if(!pkey) {
    BIO_free(bio);
    free(file_bytes);
    return -1;
  }
  len = i2d_PUBKEY(pkey, NULL);
  if(len <= 0) {
    EVP_PKEY_free(pkey);
    BIO_free(bio);
    free(file_bytes);
    return -1;
  }
  *out = malloc((size_t)len);
  if(!*out) {
    EVP_PKEY_free(pkey);
    BIO_free(bio);
    free(file_bytes);
    return -1;
  }
  {
    unsigned char *tmp = *out;
    if(i2d_PUBKEY(pkey, &tmp) != len) {
      free(*out);
      *out = NULL;
      EVP_PKEY_free(pkey);
      BIO_free(bio);
      free(file_bytes);
      return -1;
    }
  }
  *out_len = len;
  EVP_PKEY_free(pkey);
  BIO_free(bio);
  free(file_bytes);
  return 0;
}

static int openssl_check_pinned_key(SSL *ssl, const char *spec)
{
  X509 *cert;
  unsigned char *peer_der = NULL;
  int peer_len = 0;
  int ok = -1;

  if(!spec || !*spec)
    return 0;

  cert = SSL_get1_peer_certificate(ssl);
  if(!cert)
    return -1;

  if(openssl_export_pubkey_der(cert, &peer_der, &peer_len) == 0) {
    if(!strncmp(spec, "sha256//", 8)) {
      unsigned char digest[SHA256_DIGEST_LENGTH];
      char encoded[128];
      SHA256(peer_der, (size_t)peer_len, digest);
      base64_encode(digest, sizeof(digest), encoded, sizeof(encoded));
      ok = strcmp(encoded, spec + 8) ? -1 : 0;
    }
    else {
      unsigned char *pinned_der = NULL;
      int pinned_len = 0;
      if(openssl_load_pinned_pubkey_der(spec, &pinned_der, &pinned_len) == 0) {
        ok = (peer_len == pinned_len && !memcmp(peer_der, pinned_der, (size_t)peer_len)) ? 0 : -1;
        free(pinned_der);
      }
    }
  }

  free(peer_der);
  X509_free(cert);
  return ok;
}

static char *openssl_name_string(X509_NAME *name)
{
  BIO *bio;
  char *data = NULL;
  long len = 0;
  char *out;

  if(!name)
    return NULL;
  bio = BIO_new(BIO_s_mem());
  if(!bio)
    return NULL;
  if(X509_NAME_print_ex(bio, name, 0, XN_FLAG_RFC2253) < 0) {
    BIO_free(bio);
    return NULL;
  }
  len = BIO_get_mem_data(bio, &data);
  if(len <= 0) {
    BIO_free(bio);
    return NULL;
  }
  out = malloc((size_t)len + 1);
  if(out) {
    memcpy(out, data, (size_t)len);
    out[len] = '\0';
  }
  BIO_free(bio);
  return out;
}

static char *openssl_time_string(const ASN1_TIME *time_value)
{
  BIO *bio;
  char *data = NULL;
  long len = 0;
  char *out;

  if(!time_value)
    return NULL;
  bio = BIO_new(BIO_s_mem());
  if(!bio)
    return NULL;
  if(ASN1_TIME_print(bio, time_value) != 1) {
    BIO_free(bio);
    return NULL;
  }
  len = BIO_get_mem_data(bio, &data);
  if(len <= 0) {
    BIO_free(bio);
    return NULL;
  }
  out = malloc((size_t)len + 1);
  if(out) {
    memcpy(out, data, (size_t)len);
    out[len] = '\0';
  }
  BIO_free(bio);
  return out;
}

unsigned char *port_safe_tls_certinfo(struct safe_tls_connection *conn,
                                      size_t *out_len)
{
  struct certinfo_builder builder = { 0 };
  X509 *cert = NULL;
  char *subject = NULL;
  char *issuer = NULL;
  char *start = NULL;
  char *expire = NULL;
  char version[32];
  const char *sigalg = NULL;
  const char *pubalg = NULL;
  ASN1_INTEGER *serial = NULL;
  BIGNUM *bn = NULL;
  char *serial_hex = NULL;
  EVP_PKEY *pkey = NULL;

  if(out_len)
    *out_len = 0;
  if(!conn || !conn->ssl)
    return NULL;

  cert = SSL_get1_peer_certificate(conn->ssl);
  if(!cert)
    return NULL;

  subject = openssl_name_string(X509_get_subject_name(cert));
  issuer = openssl_name_string(X509_get_issuer_name(cert));
  start = openssl_time_string(X509_get0_notBefore(cert));
  expire = openssl_time_string(X509_get0_notAfter(cert));
  snprintf(version, sizeof(version), "%ld", X509_get_version(cert) + 1);
  sigalg = OBJ_nid2ln(X509_get_signature_nid(cert));
  serial = X509_get_serialNumber(cert);
  if(serial) {
    bn = ASN1_INTEGER_to_BN(serial, NULL);
    if(bn)
      serial_hex = BN_bn2hex(bn);
  }
  pkey = X509_get_pubkey(cert);
  if(pkey)
    pubalg = OBJ_nid2sn(EVP_PKEY_id(pkey));

  if(append_certinfo_line(&builder, 0, "Subject", subject) ||
     append_certinfo_line(&builder, 0, "Issuer", issuer) ||
     append_certinfo_line(&builder, 0, "Version", version) ||
     append_certinfo_line(&builder, 0, "Serial Number", serial_hex) ||
     append_certinfo_line(&builder, 0, "Signature Algorithm", sigalg) ||
     append_certinfo_line(&builder, 0, "Public Key Algorithm", pubalg) ||
     append_certinfo_line(&builder, 0, "Start date", start) ||
     append_certinfo_line(&builder, 0, "Expire date", expire)) {
    certinfo_builder_cleanup(&builder);
  }

  EVP_PKEY_free(pkey);
  OPENSSL_free(serial_hex);
  BN_free(bn);
  free(subject);
  free(issuer);
  free(start);
  free(expire);
  X509_free(cert);
  return certinfo_take(&builder, out_len);
}

int port_safe_tls_connect(int fd,
                          const char *host,
                          int verify_peer,
                          int verify_host,
                          int enable_alpn,
                          const char *pinned_public_key,
                          const unsigned char *session_data,
                          size_t session_len,
                          struct safe_tls_connection **out,
                          unsigned char **out_session_data,
                          size_t *out_session_len,
                          char *errbuf,
                          size_t errlen)
{
  struct safe_tls_connection *conn = NULL;
  SSL_SESSION *session = NULL;
  const unsigned char *session_cursor = session_data;

  *out = NULL;
  *out_session_data = NULL;
  *out_session_len = 0;
  {
    SSL_CTX *ctx = openssl_shared_ctx(verify_peer);
    if(!ctx) {
      set_error(errbuf, errlen, "SSL_CTX_new failed");
      return -1;
    }

    conn = calloc(1, sizeof(*conn));
    if(!conn) {
      set_error(errbuf, errlen, "out of memory");
      return -1;
    }

    conn->ssl = SSL_new(ctx);
  }
  if(!conn->ssl) {
    set_error(errbuf, errlen, "SSL_new failed");
    free(conn);
    return -1;
  }

  if(host && *host) {
    SSL_set_tlsext_host_name(conn->ssl, host);
    if(verify_host)
      SSL_set1_host(conn->ssl, host);
  }
  if(enable_alpn == 2) {
    static const unsigned char alpn_h2_h11[] = {
      2, 'h', '2',
      8, 'h', 't', 't', 'p', '/', '1', '.', '1'
    };
    SSL_set_alpn_protos(conn->ssl, alpn_h2_h11, sizeof(alpn_h2_h11));
  }
  else if(enable_alpn == 1) {
    static const unsigned char alpn_h11[] = {
      8, 'h', 't', 't', 'p', '/', '1', '.', '1'
    };
    SSL_set_alpn_protos(conn->ssl, alpn_h11, sizeof(alpn_h11));
  }
  if(session_data && session_len) {
    session = d2i_SSL_SESSION(NULL, &session_cursor, (long)session_len);
    if(session) {
      SSL_set_session(conn->ssl, session);
      SSL_SESSION_free(session);
    }
  }
  if(SSL_set_fd(conn->ssl, fd) != 1) {
    set_error(errbuf, errlen, "SSL_set_fd failed");
    SSL_free(conn->ssl);
    free(conn);
    return -1;
  }
  if(SSL_connect(conn->ssl) != 1) {
    char error_text[256];
    ERR_error_string_n(ERR_get_error(), error_text, sizeof(error_text));
    set_error(errbuf, errlen, error_text);
    SSL_free(conn->ssl);
    free(conn);
    return -1;
  }
  if(openssl_check_pinned_key(conn->ssl, pinned_public_key)) {
    set_error(errbuf, errlen, "SSL public key does not match pinned public key");
    SSL_free(conn->ssl);
    free(conn);
    return -1;
  }
  {
    const unsigned char *protocol = NULL;
    unsigned int protocol_len = 0;
    SSL_get0_alpn_selected(conn->ssl, &protocol, &protocol_len);
    conn->negotiated_alpn = safe_tls_map_alpn(protocol, protocol_len);
  }

  openssl_export_session(conn->ssl, out_session_data, out_session_len);

  *out = conn;
  return 0;
}

ssize_t port_safe_tls_read(struct safe_tls_connection *conn, void *buf, size_t len)
{
  int rc;
  int err;
  if(!conn || !buf)
    return -1;
  rc = SSL_read(conn->ssl, buf, (int)len);
  if(rc > 0)
    return (ssize_t)rc;
  err = SSL_get_error(conn->ssl, rc);
  if(err == SSL_ERROR_ZERO_RETURN)
    return 0;
  if(err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
    errno = EAGAIN;
    return -1;
  }
  errno = EIO;
  return -1;
}

ssize_t port_safe_tls_write(struct safe_tls_connection *conn, const void *buf, size_t len)
{
  int rc;
  int err;
  if(!conn || !buf)
    return -1;
  rc = SSL_write(conn->ssl, buf, (int)len);
  if(rc > 0)
    return (ssize_t)rc;
  err = SSL_get_error(conn->ssl, rc);
  if(err == SSL_ERROR_ZERO_RETURN)
    return 0;
  if(err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
    errno = EAGAIN;
    return -1;
  }
  errno = EIO;
  return -1;
}

int port_safe_tls_export_session(struct safe_tls_connection *conn,
                                 unsigned char **out_session_data,
                                 size_t *out_session_len)
{
  if(!out_session_data || !out_session_len)
    return -1;
  return openssl_export_session(conn ? conn->ssl : NULL,
                                out_session_data,
                                out_session_len);
}

int port_safe_tls_negotiated_alpn(struct safe_tls_connection *conn)
{
  if(!conn)
    return 0;
  return conn->negotiated_alpn;
}

void port_safe_tls_close(struct safe_tls_connection *conn)
{
  if(!conn)
    return;
  if(conn->ssl) {
    SSL_shutdown(conn->ssl);
    SSL_free(conn->ssl);
  }
  free(conn);
}

#endif

#ifdef SAFE_TLS_GNUTLS

struct safe_tls_connection {
  gnutls_certificate_credentials_t creds;
  gnutls_session_t session;
  int negotiated_alpn;
};

static pthread_once_t gnutls_once = PTHREAD_ONCE_INIT;

static int gnutls_export_session(struct safe_tls_connection *conn,
                                 unsigned char **out_session_data,
                                 size_t *out_session_len)
{
  gnutls_datum_t encoded = { 0 };
  int rc;

  *out_session_data = NULL;
  *out_session_len = 0;
  if(!conn)
    return -1;

  rc = gnutls_session_get_data2(conn->session, &encoded);
  if(rc < 0 || !encoded.data || !encoded.size)
    return -1;

  *out_session_data = malloc(encoded.size);
  if(*out_session_data) {
    memcpy(*out_session_data, encoded.data, encoded.size);
    *out_session_len = encoded.size;
  }
  gnutls_free(encoded.data);
  return (*out_session_data && *out_session_len) ? 0 : -1;
}

static void gnutls_global_init_once(void)
{
  gnutls_global_init();
}

static void gnutls_init_once(void)
{
  pthread_once(&gnutls_once, gnutls_global_init_once);
}

static int gnutls_export_pubkey_der(gnutls_x509_crt_t cert,
                                    unsigned char **out, size_t *out_len)
{
  gnutls_pubkey_t pubkey;
  size_t needed = 0;
  int rc;

  *out = NULL;
  *out_len = 0;
  rc = gnutls_pubkey_init(&pubkey);
  if(rc < 0)
    return -1;
  rc = gnutls_pubkey_import_x509(pubkey, cert, 0);
  if(rc < 0) {
    gnutls_pubkey_deinit(pubkey);
    return -1;
  }
  rc = gnutls_pubkey_export(pubkey, GNUTLS_X509_FMT_DER, NULL, &needed);
  if(rc != GNUTLS_E_SHORT_MEMORY_BUFFER) {
    gnutls_pubkey_deinit(pubkey);
    return -1;
  }
  *out = malloc(needed);
  if(!*out) {
    gnutls_pubkey_deinit(pubkey);
    return -1;
  }
  rc = gnutls_pubkey_export(pubkey, GNUTLS_X509_FMT_DER, *out, &needed);
  gnutls_pubkey_deinit(pubkey);
  if(rc < 0) {
    free(*out);
    *out = NULL;
    return -1;
  }
  *out_len = needed;
  return 0;
}

static int gnutls_load_pinned_pubkey_der(const char *spec,
                                         unsigned char **out, size_t *out_len)
{
  unsigned char *file_bytes = NULL;
  size_t file_len = 0;
  gnutls_datum_t datum;
  gnutls_pubkey_t pubkey;
  gnutls_x509_crt_t cert = NULL;
  size_t needed = 0;
  int rc;

  *out = NULL;
  *out_len = 0;
  if(load_file_bytes(spec, &file_bytes, &file_len))
    return -1;

  datum.data = file_bytes;
  datum.size = (unsigned int)file_len;
  rc = gnutls_pubkey_init(&pubkey);
  if(rc < 0) {
    free(file_bytes);
    return -1;
  }
  rc = gnutls_pubkey_import(pubkey, &datum, GNUTLS_X509_FMT_PEM);
  if(rc < 0)
    rc = gnutls_pubkey_import(pubkey, &datum, GNUTLS_X509_FMT_DER);
  if(rc < 0) {
    rc = gnutls_x509_crt_init(&cert);
    if(rc >= 0) {
      rc = gnutls_x509_crt_import(cert, &datum, GNUTLS_X509_FMT_PEM);
      if(rc < 0)
        rc = gnutls_x509_crt_import(cert, &datum, GNUTLS_X509_FMT_DER);
      if(rc >= 0)
        rc = gnutls_pubkey_import_x509(pubkey, cert, 0);
    }
  }
  if(cert)
    gnutls_x509_crt_deinit(cert);
  if(rc < 0) {
    gnutls_pubkey_deinit(pubkey);
    free(file_bytes);
    return -1;
  }

  rc = gnutls_pubkey_export(pubkey, GNUTLS_X509_FMT_DER, NULL, &needed);
  if(rc != GNUTLS_E_SHORT_MEMORY_BUFFER) {
    gnutls_pubkey_deinit(pubkey);
    free(file_bytes);
    return -1;
  }
  *out = malloc(needed);
  if(!*out) {
    gnutls_pubkey_deinit(pubkey);
    free(file_bytes);
    return -1;
  }
  rc = gnutls_pubkey_export(pubkey, GNUTLS_X509_FMT_DER, *out, &needed);
  gnutls_pubkey_deinit(pubkey);
  free(file_bytes);
  if(rc < 0) {
    free(*out);
    *out = NULL;
    return -1;
  }
  *out_len = needed;
  return 0;
}

static int gnutls_check_pinned_key(gnutls_session_t session, const char *spec)
{
  const gnutls_datum_t *peers;
  unsigned int peer_count = 0;
  gnutls_x509_crt_t cert = NULL;
  unsigned char *peer_der = NULL;
  size_t peer_len = 0;
  int rc;
  int ok = -1;

  if(!spec || !*spec)
    return 0;

  peers = gnutls_certificate_get_peers(session, &peer_count);
  if(!peers || !peer_count)
    return -1;

  rc = gnutls_x509_crt_init(&cert);
  if(rc < 0)
    return -1;
  rc = gnutls_x509_crt_import(cert, &peers[0], GNUTLS_X509_FMT_DER);
  if(rc < 0) {
    gnutls_x509_crt_deinit(cert);
    return -1;
  }

  if(gnutls_export_pubkey_der(cert, &peer_der, &peer_len) == 0) {
    if(!strncmp(spec, "sha256//", 8)) {
      unsigned char digest[32];
      char encoded[128];
      gnutls_hash_fast(GNUTLS_DIG_SHA256, peer_der, peer_len, digest);
      base64_encode(digest, sizeof(digest), encoded, sizeof(encoded));
      ok = strcmp(encoded, spec + 8) ? -1 : 0;
    }
    else {
      unsigned char *pinned_der = NULL;
      size_t pinned_len = 0;
      if(gnutls_load_pinned_pubkey_der(spec, &pinned_der, &pinned_len) == 0) {
        ok = (peer_len == pinned_len && !memcmp(peer_der, pinned_der, peer_len)) ? 0 : -1;
        free(pinned_der);
      }
    }
  }

  free(peer_der);
  gnutls_x509_crt_deinit(cert);
  return ok;
}

static int gnutls_verify_hostname_only(gnutls_session_t session, const char *host)
{
  const gnutls_datum_t *peers;
  unsigned int peer_count = 0;
  gnutls_x509_crt_t cert = NULL;
  int ok = -1;

  if(!host || !*host)
    return 0;

  peers = gnutls_certificate_get_peers(session, &peer_count);
  if(!peers || peer_count == 0)
    return -1;

  if(gnutls_x509_crt_init(&cert) < 0)
    return -1;
  if(gnutls_x509_crt_import(cert, &peers[0], GNUTLS_X509_FMT_DER) < 0) {
    gnutls_x509_crt_deinit(cert);
    return -1;
  }

  ok = gnutls_x509_crt_check_hostname(cert, host) ? 0 : -1;
  gnutls_x509_crt_deinit(cert);
  return ok;
}

unsigned char *port_safe_tls_certinfo(struct safe_tls_connection *conn,
                                      size_t *out_len)
{
  struct certinfo_builder builder = { 0 };
  const gnutls_datum_t *peers;
  unsigned int peer_count = 0;
  gnutls_x509_crt_t cert = NULL;
  char text[512];
  unsigned char serial[128];
  size_t serial_len = sizeof(serial);
  size_t text_len;
  char serial_hex[sizeof(serial) * 2 + 1];
  unsigned int i;

  if(out_len)
    *out_len = 0;
  if(!conn)
    return NULL;

  peers = gnutls_certificate_get_peers(conn->session, &peer_count);
  if(!peers || !peer_count)
    return NULL;
  if(gnutls_x509_crt_init(&cert) < 0)
    return NULL;
  if(gnutls_x509_crt_import(cert, &peers[0], GNUTLS_X509_FMT_DER) < 0) {
    gnutls_x509_crt_deinit(cert);
    return NULL;
  }

  text_len = sizeof(text);
  if(gnutls_x509_crt_get_dn(cert, text, &text_len) == 0 &&
     append_certinfo_line(&builder, 0, "Subject", text)) {
    certinfo_builder_cleanup(&builder);
    gnutls_x509_crt_deinit(cert);
    return NULL;
  }
  text_len = sizeof(text);
  if(gnutls_x509_crt_get_issuer_dn(cert, text, &text_len) == 0 &&
     append_certinfo_line(&builder, 0, "Issuer", text)) {
    certinfo_builder_cleanup(&builder);
    gnutls_x509_crt_deinit(cert);
    return NULL;
  }
  snprintf(text, sizeof(text), "%u", gnutls_x509_crt_get_version(cert));
  if(append_certinfo_line(&builder, 0, "Version", text)) {
    certinfo_builder_cleanup(&builder);
    gnutls_x509_crt_deinit(cert);
    return NULL;
  }
  serial_len = sizeof(serial);
  if(gnutls_x509_crt_get_serial(cert, serial, &serial_len) == 0) {
    for(i = 0; i < serial_len; ++i)
      snprintf(serial_hex + i * 2, sizeof(serial_hex) - i * 2, "%02x", serial[i]);
    if(append_certinfo_line(&builder, 0, "Serial Number", serial_hex)) {
      certinfo_builder_cleanup(&builder);
      gnutls_x509_crt_deinit(cert);
      return NULL;
    }
  }
  if(append_certinfo_line(
       &builder, 0, "Signature Algorithm",
       gnutls_sign_get_name(gnutls_x509_crt_get_signature_algorithm(cert))) ||
     append_certinfo_line(
       &builder, 0, "Public Key Algorithm",
       gnutls_pk_algorithm_get_name(
         gnutls_x509_crt_get_pk_algorithm(cert, NULL))) ||
     append_certinfo_time(&builder, 0, "Start date",
                          gnutls_x509_crt_get_activation_time(cert)) ||
     append_certinfo_time(&builder, 0, "Expire date",
                          gnutls_x509_crt_get_expiration_time(cert))) {
    certinfo_builder_cleanup(&builder);
    gnutls_x509_crt_deinit(cert);
    return NULL;
  }

  gnutls_x509_crt_deinit(cert);
  return certinfo_take(&builder, out_len);
}

int port_safe_tls_connect(int fd,
                          const char *host,
                          int verify_peer,
                          int verify_host,
                          int enable_alpn,
                          const char *pinned_public_key,
                          const unsigned char *session_data,
                          size_t session_len,
                          struct safe_tls_connection **out,
                          unsigned char **out_session_data,
                          size_t *out_session_len,
                          char *errbuf,
                          size_t errlen)
{
  struct safe_tls_connection *conn = NULL;
  int rc;

  *out = NULL;
  *out_session_data = NULL;
  *out_session_len = 0;
  gnutls_init_once();

  conn = calloc(1, sizeof(*conn));
  if(!conn) {
    set_error(errbuf, errlen, "out of memory");
    return -1;
  }
  rc = gnutls_certificate_allocate_credentials(&conn->creds);
  if(rc < 0) {
    set_error(errbuf, errlen, gnutls_strerror(rc));
    free(conn);
    return -1;
  }
  if(verify_peer)
    gnutls_certificate_set_x509_system_trust(conn->creds);

  rc = gnutls_init(&conn->session, GNUTLS_CLIENT);
  if(rc < 0) {
    set_error(errbuf, errlen, gnutls_strerror(rc));
    gnutls_certificate_free_credentials(conn->creds);
    free(conn);
    return -1;
  }
  rc = gnutls_set_default_priority(conn->session);
  if(rc < 0) {
    set_error(errbuf, errlen, gnutls_strerror(rc));
    gnutls_deinit(conn->session);
    gnutls_certificate_free_credentials(conn->creds);
    free(conn);
    return -1;
  }
  if(enable_alpn == 2) {
    gnutls_datum_t protocols[2];
    protocols[0].data = (unsigned char *)"h2";
    protocols[0].size = 2;
    protocols[1].data = (unsigned char *)"http/1.1";
    protocols[1].size = 8;
    gnutls_alpn_set_protocols(conn->session, protocols, 2, 0);
  }
  else if(enable_alpn == 1) {
    gnutls_datum_t protocol;
    protocol.data = (unsigned char *)"http/1.1";
    protocol.size = 8;
    gnutls_alpn_set_protocols(conn->session, &protocol, 1, 0);
  }
  if(host && *host)
    gnutls_server_name_set(conn->session, GNUTLS_NAME_DNS, host, strlen(host));
  gnutls_credentials_set(conn->session, GNUTLS_CRD_CERTIFICATE, conn->creds);
  gnutls_transport_set_int(conn->session, fd);
  if(session_data && session_len)
    gnutls_session_set_data(conn->session, session_data, session_len);

  do {
    rc = gnutls_handshake(conn->session);
  } while(rc == GNUTLS_E_INTERRUPTED || rc == GNUTLS_E_AGAIN);
  if(rc < 0) {
    set_error(errbuf, errlen, gnutls_strerror(rc));
    gnutls_deinit(conn->session);
    gnutls_certificate_free_credentials(conn->creds);
    free(conn);
    return -1;
  }

  if(verify_peer) {
    unsigned int verify_status = 0;
    rc = gnutls_certificate_verify_peers3(conn->session,
                                          (verify_host && host && *host) ? host : NULL,
                                          &verify_status);
    if(rc < 0 || verify_status != 0) {
      set_error(errbuf, errlen, "TLS peer verification failed");
      gnutls_bye(conn->session, GNUTLS_SHUT_RDWR);
      gnutls_deinit(conn->session);
      gnutls_certificate_free_credentials(conn->creds);
      free(conn);
      return -1;
    }
  }
  else if(verify_host && gnutls_verify_hostname_only(conn->session, host)) {
    set_error(errbuf, errlen, "TLS peer hostname verification failed");
    gnutls_bye(conn->session, GNUTLS_SHUT_RDWR);
    gnutls_deinit(conn->session);
    gnutls_certificate_free_credentials(conn->creds);
    free(conn);
    return -1;
  }

  if(gnutls_check_pinned_key(conn->session, pinned_public_key)) {
    set_error(errbuf, errlen, "SSL public key does not match pinned public key");
    gnutls_bye(conn->session, GNUTLS_SHUT_RDWR);
    gnutls_deinit(conn->session);
    gnutls_certificate_free_credentials(conn->creds);
    free(conn);
    return -1;
  }
  {
    gnutls_datum_t protocol = { 0 };
    if(gnutls_alpn_get_selected_protocol(conn->session, &protocol) == 0) {
      conn->negotiated_alpn = safe_tls_map_alpn(protocol.data, protocol.size);
    }
  }

  gnutls_export_session(conn, out_session_data, out_session_len);

  *out = conn;
  return 0;
}

ssize_t port_safe_tls_read(struct safe_tls_connection *conn, void *buf, size_t len)
{
  ssize_t rc;
  if(!conn || !buf)
    return -1;
  do {
    rc = gnutls_record_recv(conn->session, buf, len);
  } while(rc == GNUTLS_E_INTERRUPTED);
  if(rc >= 0)
    return rc;
  if(rc == GNUTLS_E_AGAIN) {
    errno = EAGAIN;
    return -1;
  }
  errno = EIO;
  return -1;
}

ssize_t port_safe_tls_write(struct safe_tls_connection *conn, const void *buf, size_t len)
{
  ssize_t rc;
  if(!conn || !buf)
    return -1;
  do {
    rc = gnutls_record_send(conn->session, buf, len);
  } while(rc == GNUTLS_E_INTERRUPTED);
  if(rc >= 0)
    return rc;
  if(rc == GNUTLS_E_AGAIN) {
    errno = EAGAIN;
    return -1;
  }
  errno = EIO;
  return -1;
}

int port_safe_tls_export_session(struct safe_tls_connection *conn,
                                 unsigned char **out_session_data,
                                 size_t *out_session_len)
{
  if(!out_session_data || !out_session_len)
    return -1;
  return gnutls_export_session(conn, out_session_data, out_session_len);
}

int port_safe_tls_negotiated_alpn(struct safe_tls_connection *conn)
{
  if(!conn)
    return 0;
  return conn->negotiated_alpn;
}

void port_safe_tls_close(struct safe_tls_connection *conn)
{
  if(!conn)
    return;
  (void)gnutls_bye(conn->session, GNUTLS_SHUT_WR);
  gnutls_deinit(conn->session);
  gnutls_certificate_free_credentials(conn->creds);
  free(conn);
}

#endif

void port_safe_tls_free_bytes(unsigned char *ptr)
{
  free(ptr);
}
