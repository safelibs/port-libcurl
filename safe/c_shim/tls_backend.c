#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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

#ifdef SAFE_TLS_OPENSSL

struct safe_tls_connection {
  SSL_CTX *ctx;
  SSL *ssl;
};

static void openssl_init_once(void)
{
  static int initialized = 0;
  if(!initialized) {
    OPENSSL_init_ssl(0, NULL);
    initialized = 1;
  }
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

int curl_safe_tls_connect(int fd,
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
  unsigned char *encoded = NULL;
  int encoded_len;

  *out = NULL;
  *out_session_data = NULL;
  *out_session_len = 0;
  openssl_init_once();

  conn = calloc(1, sizeof(*conn));
  if(!conn) {
    set_error(errbuf, errlen, "out of memory");
    return -1;
  }
  conn->ctx = SSL_CTX_new(TLS_client_method());
  if(!conn->ctx) {
    set_error(errbuf, errlen, "SSL_CTX_new failed");
    free(conn);
    return -1;
  }
  SSL_CTX_set_verify(conn->ctx, verify_peer ? SSL_VERIFY_PEER : SSL_VERIFY_NONE, NULL);
  if(verify_peer)
    SSL_CTX_set_default_verify_paths(conn->ctx);

  conn->ssl = SSL_new(conn->ctx);
  if(!conn->ssl) {
    set_error(errbuf, errlen, "SSL_new failed");
    SSL_CTX_free(conn->ctx);
    free(conn);
    return -1;
  }

  if(host && *host) {
    SSL_set_tlsext_host_name(conn->ssl, host);
    if(verify_host)
      SSL_set1_host(conn->ssl, host);
  }
  if(enable_alpn) {
    static const unsigned char alpn_h11[] = { 8, 'h', 't', 't', 'p', '/', '1', '.', '1' };
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
    SSL_CTX_free(conn->ctx);
    free(conn);
    return -1;
  }
  if(SSL_connect(conn->ssl) != 1) {
    char error_text[256];
    ERR_error_string_n(ERR_get_error(), error_text, sizeof(error_text));
    set_error(errbuf, errlen, error_text);
    SSL_free(conn->ssl);
    SSL_CTX_free(conn->ctx);
    free(conn);
    return -1;
  }
  if(openssl_check_pinned_key(conn->ssl, pinned_public_key)) {
    set_error(errbuf, errlen, "SSL public key does not match pinned public key");
    SSL_free(conn->ssl);
    SSL_CTX_free(conn->ctx);
    free(conn);
    return -1;
  }

  session = SSL_get1_session(conn->ssl);
  if(session) {
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
  }

  *out = conn;
  return 0;
}

ssize_t curl_safe_tls_read(struct safe_tls_connection *conn, void *buf, size_t len)
{
  int rc;
  if(!conn || !buf)
    return -1;
  rc = SSL_read(conn->ssl, buf, (int)len);
  return rc > 0 ? (ssize_t)rc : -1;
}

ssize_t curl_safe_tls_write(struct safe_tls_connection *conn, const void *buf, size_t len)
{
  int rc;
  if(!conn || !buf)
    return -1;
  rc = SSL_write(conn->ssl, buf, (int)len);
  return rc > 0 ? (ssize_t)rc : -1;
}

void curl_safe_tls_close(struct safe_tls_connection *conn)
{
  if(!conn)
    return;
  if(conn->ssl) {
    SSL_shutdown(conn->ssl);
    SSL_free(conn->ssl);
  }
  if(conn->ctx)
    SSL_CTX_free(conn->ctx);
  free(conn);
}

#endif

#ifdef SAFE_TLS_GNUTLS

struct safe_tls_connection {
  gnutls_certificate_credentials_t creds;
  gnutls_session_t session;
};

static void gnutls_init_once(void)
{
  static int initialized = 0;
  if(!initialized) {
    gnutls_global_init();
    initialized = 1;
  }
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

int curl_safe_tls_connect(int fd,
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
  gnutls_datum_t encoded = { 0 };
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
  if(enable_alpn) {
    gnutls_datum_t protocols[1];
    protocols[0].data = (unsigned char *)"http/1.1";
    protocols[0].size = 8;
    gnutls_alpn_set_protocols(conn->session, protocols, 1, 0);
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

  if(verify_peer || verify_host) {
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

  if(gnutls_check_pinned_key(conn->session, pinned_public_key)) {
    set_error(errbuf, errlen, "SSL public key does not match pinned public key");
    gnutls_bye(conn->session, GNUTLS_SHUT_RDWR);
    gnutls_deinit(conn->session);
    gnutls_certificate_free_credentials(conn->creds);
    free(conn);
    return -1;
  }

  rc = gnutls_session_get_data2(conn->session, &encoded);
  if(rc >= 0 && encoded.data && encoded.size) {
    *out_session_data = malloc(encoded.size);
    if(*out_session_data) {
      memcpy(*out_session_data, encoded.data, encoded.size);
      *out_session_len = encoded.size;
    }
    gnutls_free(encoded.data);
  }

  *out = conn;
  return 0;
}

ssize_t curl_safe_tls_read(struct safe_tls_connection *conn, void *buf, size_t len)
{
  ssize_t rc;
  if(!conn || !buf)
    return -1;
  do {
    rc = gnutls_record_recv(conn->session, buf, len);
  } while(rc == GNUTLS_E_INTERRUPTED);
  return rc >= 0 ? rc : -1;
}

ssize_t curl_safe_tls_write(struct safe_tls_connection *conn, const void *buf, size_t len)
{
  ssize_t rc;
  if(!conn || !buf)
    return -1;
  do {
    rc = gnutls_record_send(conn->session, buf, len);
  } while(rc == GNUTLS_E_INTERRUPTED);
  return rc >= 0 ? rc : -1;
}

void curl_safe_tls_close(struct safe_tls_connection *conn)
{
  if(!conn)
    return;
  gnutls_bye(conn->session, GNUTLS_SHUT_RDWR);
  gnutls_deinit(conn->session);
  gnutls_certificate_free_credentials(conn->creds);
  free(conn);
}

#endif

void curl_safe_tls_free_bytes(unsigned char *ptr)
{
  free(ptr);
}
