#include <stdio.h>
#include <string.h>

#include <libssh2.h>
#include <libssh2_sftp.h>

typedef ssize_t (*curl_safe_ssh_write_callback)(const char *buf, size_t len,
                                                void *ctx);

enum {
  CURL_SAFE_SSH_OK = 0,
  CURL_SAFE_SSH_CONNECT = 1,
  CURL_SAFE_SSH_AUTH = 2,
  CURL_SAFE_SSH_REMOTE_NOT_FOUND = 3,
  CURL_SAFE_SSH_REMOTE_ACCESS = 4,
  CURL_SAFE_SSH_SEND = 5,
  CURL_SAFE_SSH_RECV = 6,
  CURL_SAFE_SSH_CALLBACK = 7
};

static void set_error(char *errbuf, size_t errlen, const char *msg)
{
  if(!errbuf || !errlen)
    return;
  if(!msg)
    msg = "unknown SSH error";
  snprintf(errbuf, errlen, "%s", msg);
}

static void ssh_init_once(void)
{
  static int initialized = 0;
  if(!initialized) {
    libssh2_init(0);
    initialized = 1;
  }
}

static int set_session_error(LIBSSH2_SESSION *session, int fallback,
                             char *errbuf, size_t errlen)
{
  char *message = NULL;
  int message_len = 0;
  int rc = session ? libssh2_session_last_error(session, &message, &message_len, 0)
                   : 0;
  if(message && message_len > 0)
    set_error(errbuf, errlen, message);
  else
    set_error(errbuf, errlen, "SSH session failed");

  if(rc == LIBSSH2_ERROR_SCP_PROTOCOL)
    return CURL_SAFE_SSH_REMOTE_NOT_FOUND;
  if(rc == LIBSSH2_ERROR_SOCKET_SEND)
    return CURL_SAFE_SSH_SEND;
  if(rc == LIBSSH2_ERROR_SOCKET_RECV)
    return CURL_SAFE_SSH_RECV;
  return fallback;
}

static int authenticate_password(LIBSSH2_SESSION *session, const char *username,
                                 const char *password, char *errbuf,
                                 size_t errlen)
{
  int rc;

  if(!username || !*username) {
    set_error(errbuf, errlen, "missing SSH username");
    return CURL_SAFE_SSH_AUTH;
  }

  rc = libssh2_userauth_password_ex(session, username, (unsigned int)strlen(username),
                                    password ? password : "",
                                    (unsigned int)(password ? strlen(password) : 0),
                                    NULL);
  if(rc) {
    set_error(errbuf, errlen, "SSH authentication failed");
    return CURL_SAFE_SSH_AUTH;
  }
  return CURL_SAFE_SSH_OK;
}

static int map_sftp_error(unsigned long code, char *errbuf, size_t errlen)
{
  switch(code) {
  case LIBSSH2_FX_NO_SUCH_FILE:
  case LIBSSH2_FX_NO_SUCH_PATH:
    set_error(errbuf, errlen, "SSH server could not find the requested path");
    return CURL_SAFE_SSH_REMOTE_NOT_FOUND;
  case LIBSSH2_FX_PERMISSION_DENIED:
  case LIBSSH2_FX_WRITE_PROTECT:
  case LIBSSH2_FX_LOCK_CONFlICT:
    set_error(errbuf, errlen, "SSH server denied access to the requested path");
    return CURL_SAFE_SSH_REMOTE_ACCESS;
  default:
    set_error(errbuf, errlen, "SFTP transfer failed");
    return CURL_SAFE_SSH_RECV;
  }
}

static int scp_download(LIBSSH2_SESSION *session, const char *path,
                        curl_safe_ssh_write_callback write_cb, void *write_ctx,
                        unsigned long long *transferred, char *errbuf,
                        size_t errlen)
{
  LIBSSH2_CHANNEL *channel;
  libssh2_struct_stat fileinfo;
  char buffer[32768];

  channel = libssh2_scp_recv2(session, path, &fileinfo);
  if(!channel)
    return set_session_error(session, CURL_SAFE_SSH_REMOTE_NOT_FOUND,
                             errbuf, errlen);

  for(;;) {
    ssize_t rc = libssh2_channel_read(channel, buffer, sizeof(buffer));
    if(rc > 0) {
      if(write_cb && write_cb(buffer, (size_t)rc, write_ctx) < 0) {
        set_error(errbuf, errlen, "SSH write callback failed");
        libssh2_channel_free(channel);
        return CURL_SAFE_SSH_CALLBACK;
      }
      if(transferred)
        *transferred += (unsigned long long)rc;
      continue;
    }
    if(rc == 0)
      break;
    libssh2_channel_free(channel);
    return set_session_error(session, CURL_SAFE_SSH_RECV, errbuf, errlen);
  }

  libssh2_channel_send_eof(channel);
  libssh2_channel_wait_eof(channel);
  libssh2_channel_close(channel);
  libssh2_channel_wait_closed(channel);
  libssh2_channel_free(channel);
  return CURL_SAFE_SSH_OK;
}

static int scp_upload(LIBSSH2_SESSION *session, const char *path,
                      const unsigned char *upload_data, size_t upload_len,
                      unsigned long long *transferred, char *errbuf,
                      size_t errlen)
{
  LIBSSH2_CHANNEL *channel;
  size_t sent = 0;

  channel = libssh2_scp_send64(session, path, 0644,
                               (libssh2_int64_t)upload_len, 0, 0);
  if(!channel)
    return set_session_error(session, CURL_SAFE_SSH_REMOTE_ACCESS,
                             errbuf, errlen);

  while(sent < upload_len) {
    ssize_t rc = libssh2_channel_write(channel,
                                       (const char *)upload_data + sent,
                                       upload_len - sent);
    if(rc > 0) {
      sent += (size_t)rc;
      if(transferred)
        *transferred += (unsigned long long)rc;
      continue;
    }
    libssh2_channel_free(channel);
    return set_session_error(session, CURL_SAFE_SSH_SEND, errbuf, errlen);
  }

  libssh2_channel_send_eof(channel);
  libssh2_channel_wait_eof(channel);
  libssh2_channel_close(channel);
  libssh2_channel_wait_closed(channel);
  libssh2_channel_free(channel);
  return CURL_SAFE_SSH_OK;
}

static int sftp_download(LIBSSH2_SESSION *session, const char *path,
                         curl_safe_ssh_write_callback write_cb, void *write_ctx,
                         unsigned long long *transferred, char *errbuf,
                         size_t errlen)
{
  LIBSSH2_SFTP *sftp = NULL;
  LIBSSH2_SFTP_HANDLE *handle = NULL;
  char buffer[32768];
  int result = CURL_SAFE_SSH_RECV;

  sftp = libssh2_sftp_init(session);
  if(!sftp)
    return set_session_error(session, CURL_SAFE_SSH_RECV, errbuf, errlen);

  handle = libssh2_sftp_open_ex(sftp, path, (unsigned int)strlen(path),
                                LIBSSH2_FXF_READ, 0,
                                LIBSSH2_SFTP_OPENFILE);
  if(!handle) {
    result = map_sftp_error(libssh2_sftp_last_error(sftp), errbuf, errlen);
    libssh2_sftp_shutdown(sftp);
    return result;
  }

  for(;;) {
    ssize_t rc = libssh2_sftp_read(handle, buffer, sizeof(buffer));
    if(rc > 0) {
      if(write_cb && write_cb(buffer, (size_t)rc, write_ctx) < 0) {
        set_error(errbuf, errlen, "SSH write callback failed");
        result = CURL_SAFE_SSH_CALLBACK;
        break;
      }
      if(transferred)
        *transferred += (unsigned long long)rc;
      continue;
    }
    if(rc == 0) {
      result = CURL_SAFE_SSH_OK;
      break;
    }
    result = map_sftp_error(libssh2_sftp_last_error(sftp), errbuf, errlen);
    break;
  }

  if(handle)
    libssh2_sftp_close_handle(handle);
  if(sftp)
    libssh2_sftp_shutdown(sftp);
  return result;
}

static int sftp_upload(LIBSSH2_SESSION *session, const char *path,
                       const unsigned char *upload_data, size_t upload_len,
                       unsigned long long *transferred, char *errbuf,
                       size_t errlen)
{
  LIBSSH2_SFTP *sftp = NULL;
  LIBSSH2_SFTP_HANDLE *handle = NULL;
  size_t sent = 0;
  int result = CURL_SAFE_SSH_SEND;

  sftp = libssh2_sftp_init(session);
  if(!sftp)
    return set_session_error(session, CURL_SAFE_SSH_RECV, errbuf, errlen);

  handle = libssh2_sftp_open_ex(
    sftp, path, (unsigned int)strlen(path),
    LIBSSH2_FXF_WRITE | LIBSSH2_FXF_CREAT | LIBSSH2_FXF_TRUNC,
    LIBSSH2_SFTP_S_IRUSR | LIBSSH2_SFTP_S_IWUSR | LIBSSH2_SFTP_S_IRGRP |
      LIBSSH2_SFTP_S_IROTH,
    LIBSSH2_SFTP_OPENFILE);
  if(!handle) {
    result = map_sftp_error(libssh2_sftp_last_error(sftp), errbuf, errlen);
    libssh2_sftp_shutdown(sftp);
    return result;
  }

  while(sent < upload_len) {
    ssize_t rc = libssh2_sftp_write(handle,
                                    (const char *)upload_data + sent,
                                    upload_len - sent);
    if(rc > 0) {
      sent += (size_t)rc;
      if(transferred)
        *transferred += (unsigned long long)rc;
      continue;
    }
    result = map_sftp_error(libssh2_sftp_last_error(sftp), errbuf, errlen);
    break;
  }

  if(sent == upload_len)
    result = CURL_SAFE_SSH_OK;

  if(handle)
    libssh2_sftp_close_handle(handle);
  if(sftp)
    libssh2_sftp_shutdown(sftp);
  return result;
}

int curl_safe_ssh_transfer(int fd, const char *scheme, const char *username,
                           const char *password, const char *path, int upload,
                           const unsigned char *upload_data, size_t upload_len,
                           curl_safe_ssh_write_callback write_cb,
                           void *write_ctx,
                           unsigned long long *transferred, char *errbuf,
                           size_t errlen)
{
  LIBSSH2_SESSION *session = NULL;
  int result;

  if(transferred)
    *transferred = 0;
  if(!scheme || !path) {
    set_error(errbuf, errlen, "missing SSH scheme or path");
    return CURL_SAFE_SSH_CONNECT;
  }

  ssh_init_once();
  session = libssh2_session_init();
  if(!session) {
    set_error(errbuf, errlen, "libssh2_session_init failed");
    return CURL_SAFE_SSH_CONNECT;
  }

  libssh2_session_set_blocking(session, 1);
  if(libssh2_session_handshake(session, fd)) {
    result = set_session_error(session, CURL_SAFE_SSH_CONNECT, errbuf, errlen);
    libssh2_session_free(session);
    return result;
  }

  result = authenticate_password(session, username, password, errbuf, errlen);
  if(result != CURL_SAFE_SSH_OK) {
    libssh2_session_disconnect(session, "authentication failed");
    libssh2_session_free(session);
    return result;
  }

  if(!strcmp(scheme, "sftp"))
    result = upload ? sftp_upload(session, path, upload_data, upload_len,
                                  transferred, errbuf, errlen)
                    : sftp_download(session, path, write_cb, write_ctx,
                                    transferred, errbuf, errlen);
  else
    result = upload ? scp_upload(session, path, upload_data, upload_len,
                                 transferred, errbuf, errlen)
                    : scp_download(session, path, write_cb, write_ctx,
                                   transferred, errbuf, errlen);

  libssh2_session_disconnect(session, "transfer complete");
  libssh2_session_free(session);
  return result;
}
