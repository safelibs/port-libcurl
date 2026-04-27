#include <errno.h>
#include <nghttp2/nghttp2.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

struct safe_h2_nv {
  const uint8_t *name;
  size_t namelen;
  const uint8_t *value;
  size_t valuelen;
  uint8_t flags;
};

typedef ssize_t (*safe_h2_send_cb)(void *userp, const uint8_t *data, size_t len);
typedef ssize_t (*safe_h2_recv_cb)(void *userp, uint8_t *data, size_t len);
typedef ssize_t (*safe_h2_body_read_cb)(void *userp, uint8_t *data, size_t len, int *eof);
typedef int (*safe_h2_header_block_cb)(void *userp, int kind, const uint8_t *data,
                                       size_t len, int end_stream);
typedef int (*safe_h2_push_promise_cb)(void *userp, const uint8_t *data, size_t len);
typedef ssize_t (*safe_h2_data_cb)(void *userp, const uint8_t *data, size_t len);

#define SAFE_H2_BLOCK_RESPONSE 1
#define SAFE_H2_BLOCK_TRAILER 2
#define SAFE_H2_RECV_ERROR 56

struct safe_h2_bridge {
  safe_h2_send_cb send_cb;
  safe_h2_recv_cb recv_cb;
  safe_h2_body_read_cb body_read_cb;
  safe_h2_header_block_cb header_block_cb;
  safe_h2_push_promise_cb push_promise_cb;
  safe_h2_data_cb data_cb;
  void *userp;
  int *result_code;
  int32_t request_stream_id;
  uint32_t stream_error_code;
  int stream_closed;
  uint8_t *header_block;
  size_t header_len;
  size_t header_cap;
  size_t max_header_bytes;
  int collecting_headers;
  int saw_status;
  int header_end_stream;
  uint8_t *push_block;
  size_t push_len;
  size_t push_cap;
  int collecting_push;
  char *errbuf;
  size_t errlen;
};

static void safe_h2_set_error(struct safe_h2_bridge *bridge, const char *msg) {
  if(!bridge || !bridge->errbuf || bridge->errlen == 0) {
    return;
  }
  snprintf(bridge->errbuf, bridge->errlen, "%s", msg);
}

static void safe_h2_set_error_if_empty(struct safe_h2_bridge *bridge, const char *msg) {
  if(!bridge || !bridge->errbuf || bridge->errlen == 0 || bridge->errbuf[0] != '\0') {
    return;
  }
  snprintf(bridge->errbuf, bridge->errlen, "%s", msg);
}

static void safe_h2_set_result(struct safe_h2_bridge *bridge, int code) {
  if(bridge && bridge->result_code) {
    *bridge->result_code = code;
  }
}

static int safe_h2_limit_available(struct safe_h2_bridge *bridge, size_t used, size_t extra) {
  if(!bridge || bridge->max_header_bytes == 0) {
    return 0;
  }
  if(used > bridge->max_header_bytes || extra > bridge->max_header_bytes - used) {
    safe_h2_set_result(bridge, SAFE_H2_RECV_ERROR);
    safe_h2_set_error(bridge, "Too large response headers");
    return -1;
  }
  return 0;
}

static int safe_h2_reserve(struct safe_h2_bridge *bridge, size_t extra) {
  size_t needed;
  uint8_t *grown;

  if(!bridge) {
    return -1;
  }
  if(safe_h2_limit_available(bridge, bridge->header_len, extra) != 0) {
    return -1;
  }
  needed = bridge->header_len + extra;
  if(needed <= bridge->header_cap) {
    return 0;
  }
  if(bridge->header_cap == 0) {
    bridge->header_cap = 1024;
  }
  while(bridge->header_cap < needed) {
    bridge->header_cap *= 2;
  }
  grown = realloc(bridge->header_block, bridge->header_cap);
  if(!grown) {
    safe_h2_set_error(bridge, "out of memory growing HTTP/2 header block");
    return -1;
  }
  bridge->header_block = grown;
  return 0;
}

static int safe_h2_append_bytes(struct safe_h2_bridge *bridge, const uint8_t *data, size_t len) {
  if(len == 0) {
    return 0;
  }
  if(safe_h2_reserve(bridge, len) != 0) {
    return -1;
  }
  memcpy(bridge->header_block + bridge->header_len, data, len);
  bridge->header_len += len;
  return 0;
}

static int safe_h2_append_text(struct safe_h2_bridge *bridge, const char *text) {
  return safe_h2_append_bytes(bridge, (const uint8_t *)text, strlen(text));
}

static int safe_h2_reserve_push(struct safe_h2_bridge *bridge, size_t extra) {
  size_t needed;
  uint8_t *grown;

  if(!bridge) {
    return -1;
  }
  if(safe_h2_limit_available(bridge, bridge->push_len, extra) != 0) {
    return -1;
  }
  needed = bridge->push_len + extra;
  if(needed <= bridge->push_cap) {
    return 0;
  }
  if(bridge->push_cap == 0) {
    bridge->push_cap = 512;
  }
  while(bridge->push_cap < needed) {
    bridge->push_cap *= 2;
  }
  grown = realloc(bridge->push_block, bridge->push_cap);
  if(!grown) {
    safe_h2_set_error(bridge, "out of memory growing HTTP/2 push header block");
    return -1;
  }
  bridge->push_block = grown;
  return 0;
}

static int safe_h2_append_push_bytes(struct safe_h2_bridge *bridge, const uint8_t *data,
                                     size_t len) {
  if(len == 0) {
    return 0;
  }
  if(safe_h2_reserve_push(bridge, len) != 0) {
    return -1;
  }
  memcpy(bridge->push_block + bridge->push_len, data, len);
  bridge->push_len += len;
  return 0;
}

static int safe_h2_append_push_text(struct safe_h2_bridge *bridge, const char *text) {
  return safe_h2_append_push_bytes(bridge, (const uint8_t *)text, strlen(text));
}

static void safe_h2_reset_headers(struct safe_h2_bridge *bridge) {
  bridge->header_len = 0;
  bridge->collecting_headers = 1;
  bridge->saw_status = 0;
  bridge->header_end_stream = 0;
}

static void safe_h2_reset_push_headers(struct safe_h2_bridge *bridge) {
  bridge->push_len = 0;
  bridge->collecting_push = 1;
}

static ssize_t safe_h2_send_callback(nghttp2_session *session, const uint8_t *data,
                                     size_t length, int flags, void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  ssize_t written;
  (void)session;
  (void)flags;

  written = bridge->send_cb(bridge->userp, data, length);
  if(written < 0) {
    if(errno == EAGAIN || errno == EWOULDBLOCK) {
      return NGHTTP2_ERR_WOULDBLOCK;
    }
    safe_h2_set_error(bridge, "HTTP/2 send callback failed");
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  return written;
}

static ssize_t safe_h2_recv_callback(nghttp2_session *session, uint8_t *buf, size_t length,
                                     int flags, void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  ssize_t read_len;
  (void)session;
  (void)flags;

  read_len = bridge->recv_cb(bridge->userp, buf, length);
  if(read_len < 0) {
    if(errno == EAGAIN || errno == EWOULDBLOCK) {
      return NGHTTP2_ERR_WOULDBLOCK;
    }
    safe_h2_set_error(bridge, "HTTP/2 receive callback failed");
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  if(read_len == 0) {
    return NGHTTP2_ERR_EOF;
  }
  return read_len;
}

static ssize_t safe_h2_body_read_callback(nghttp2_session *session, int32_t stream_id,
                                          uint8_t *buf, size_t length, uint32_t *data_flags,
                                          nghttp2_data_source *source, void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  int eof = 0;
  ssize_t read_len;
  (void)session;
  (void)stream_id;
  (void)source;

  if(!bridge->body_read_cb) {
    *data_flags = NGHTTP2_DATA_FLAG_EOF;
    return 0;
  }
  read_len = bridge->body_read_cb(bridge->userp, buf, length, &eof);
  if(read_len < 0) {
    safe_h2_set_error(bridge, "HTTP/2 request body callback failed");
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  if(eof) {
    *data_flags |= NGHTTP2_DATA_FLAG_EOF;
  }
  return read_len;
}

static int safe_h2_on_begin_headers(nghttp2_session *session, const nghttp2_frame *frame,
                                    void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  (void)session;

  if(frame->hd.stream_id == bridge->request_stream_id && frame->hd.type == NGHTTP2_HEADERS) {
    safe_h2_reset_headers(bridge);
  }
  else if(frame->hd.type == NGHTTP2_PUSH_PROMISE && bridge->push_promise_cb) {
    safe_h2_reset_push_headers(bridge);
  }
  return 0;
}

static int safe_h2_on_header(nghttp2_session *session, const nghttp2_frame *frame,
                             const uint8_t *name, size_t namelen, const uint8_t *value,
                             size_t valuelen, uint8_t flags, void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  (void)session;
  (void)flags;

  if(bridge->collecting_push && frame->hd.type == NGHTTP2_PUSH_PROMISE) {
    if(safe_h2_append_push_bytes(bridge, name, namelen) != 0 ||
       safe_h2_append_push_text(bridge, ": ") != 0 ||
       safe_h2_append_push_bytes(bridge, value, valuelen) != 0 ||
       safe_h2_append_push_text(bridge, "\r\n") != 0) {
      return NGHTTP2_ERR_NOMEM;
    }
    return 0;
  }

  if(!bridge->collecting_headers || frame->hd.stream_id != bridge->request_stream_id ||
     frame->hd.type != NGHTTP2_HEADERS) {
    return 0;
  }
  if(namelen == 7 && memcmp(name, ":status", 7) == 0) {
    if(valuelen == 3) {
      if(safe_h2_append_text(bridge, "HTTP/2 ") != 0 ||
         safe_h2_append_bytes(bridge, value, valuelen) != 0 ||
         safe_h2_append_text(bridge, "\r\n") != 0) {
        return NGHTTP2_ERR_CALLBACK_FAILURE;
      }
      bridge->saw_status = 1;
    }
    return 0;
  }
  if(safe_h2_append_bytes(bridge, name, namelen) != 0 ||
     safe_h2_append_text(bridge, ": ") != 0 ||
     safe_h2_append_bytes(bridge, value, valuelen) != 0 ||
     safe_h2_append_text(bridge, "\r\n") != 0) {
    return NGHTTP2_ERR_NOMEM;
  }
  return 0;
}

static int safe_h2_on_frame_recv(nghttp2_session *session, const nghttp2_frame *frame,
                                 void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  int kind;
  int rc;
  (void)session;

  if(bridge->collecting_push && frame->hd.type == NGHTTP2_PUSH_PROMISE) {
    if(safe_h2_append_push_text(bridge, "\r\n") != 0) {
      return NGHTTP2_ERR_NOMEM;
    }
    rc = bridge->push_promise_cb(bridge->userp, bridge->push_block, bridge->push_len);
    bridge->collecting_push = 0;
    bridge->push_len = 0;
    if(rc != 0) {
      safe_h2_set_error(bridge, "HTTP/2 push promise callback failed");
      return NGHTTP2_ERR_CALLBACK_FAILURE;
    }
    return 0;
  }

  if(!bridge->collecting_headers || frame->hd.stream_id != bridge->request_stream_id ||
     frame->hd.type != NGHTTP2_HEADERS) {
    return 0;
  }

  bridge->header_end_stream = (frame->hd.flags & NGHTTP2_FLAG_END_STREAM) != 0;
  if(bridge->saw_status) {
    if(safe_h2_append_text(bridge, "\r\n") != 0) {
      return NGHTTP2_ERR_CALLBACK_FAILURE;
    }
    kind = SAFE_H2_BLOCK_RESPONSE;
    rc = bridge->header_block_cb(bridge->userp, kind, bridge->header_block, bridge->header_len,
                                 bridge->header_end_stream);
  } else {
    if(safe_h2_append_text(bridge, "\r\n") != 0) {
      return NGHTTP2_ERR_CALLBACK_FAILURE;
    }
    kind = SAFE_H2_BLOCK_TRAILER;
    rc = bridge->header_block_cb(bridge->userp, kind, bridge->header_block,
                                 bridge->header_len, bridge->header_end_stream);
  }
  bridge->collecting_headers = 0;
  bridge->header_len = 0;
  if(rc != 0) {
    safe_h2_set_error(bridge, "HTTP/2 header block callback failed");
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  return 0;
}

static int safe_h2_on_data_chunk_recv(nghttp2_session *session, uint8_t flags, int32_t stream_id,
                                      const uint8_t *data, size_t len, void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  ssize_t consumed;
  (void)session;
  (void)flags;

  if(stream_id != bridge->request_stream_id) {
    return 0;
  }
  consumed = bridge->data_cb(bridge->userp, data, len);
  if(consumed < 0 || (size_t)consumed != len) {
    safe_h2_set_error(bridge, "HTTP/2 body callback failed");
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  return 0;
}

static int safe_h2_on_stream_close(nghttp2_session *session, int32_t stream_id, uint32_t error_code,
                                   void *user_data) {
  struct safe_h2_bridge *bridge = user_data;
  (void)session;

  if(stream_id == bridge->request_stream_id) {
    bridge->stream_closed = 1;
    bridge->stream_error_code = error_code;
  }
  return 0;
}

int port_safe_http2_perform(const struct safe_h2_nv *headers, size_t header_count, int has_body,
                            int allow_push, safe_h2_send_cb send_cb, safe_h2_recv_cb recv_cb,
                            safe_h2_body_read_cb body_read_cb,
                            safe_h2_header_block_cb header_block_cb,
                            safe_h2_push_promise_cb push_promise_cb,
                            safe_h2_data_cb data_cb, void *userp, size_t max_header_bytes,
                            int *result_code, uint32_t *stream_error_code, char *errbuf,
                            size_t errlen) {
  nghttp2_session_callbacks *callbacks = NULL;
  nghttp2_session *session = NULL;
  nghttp2_settings_entry settings[1];
  nghttp2_nv *nva = NULL;
  nghttp2_data_provider data_prd;
  struct safe_h2_bridge bridge;
  int32_t stream_id;
  int rv;
  size_t i;

  if(!headers || header_count == 0 || !send_cb || !recv_cb || !header_block_cb || !data_cb) {
    if(errbuf && errlen) {
      snprintf(errbuf, errlen, "invalid HTTP/2 bridge arguments");
    }
    return -1;
  }

  memset(&bridge, 0, sizeof(bridge));
  bridge.send_cb = send_cb;
  bridge.recv_cb = recv_cb;
  bridge.body_read_cb = body_read_cb;
  bridge.header_block_cb = header_block_cb;
  bridge.push_promise_cb = push_promise_cb;
  bridge.data_cb = data_cb;
  bridge.userp = userp;
  bridge.result_code = result_code;
  bridge.max_header_bytes = max_header_bytes;
  bridge.errbuf = errbuf;
  bridge.errlen = errlen;
  if(result_code) {
    *result_code = 0;
  }
  if(errbuf && errlen) {
    errbuf[0] = '\0';
  }

  rv = nghttp2_session_callbacks_new(&callbacks);
  if(rv != 0) {
    safe_h2_set_error(&bridge, "failed to allocate nghttp2 callbacks");
    return -1;
  }
  nghttp2_session_callbacks_set_send_callback(callbacks, safe_h2_send_callback);
  nghttp2_session_callbacks_set_recv_callback(callbacks, safe_h2_recv_callback);
  nghttp2_session_callbacks_set_on_begin_headers_callback(callbacks, safe_h2_on_begin_headers);
  nghttp2_session_callbacks_set_on_header_callback(callbacks, safe_h2_on_header);
  nghttp2_session_callbacks_set_on_frame_recv_callback(callbacks, safe_h2_on_frame_recv);
  nghttp2_session_callbacks_set_on_data_chunk_recv_callback(callbacks,
                                                            safe_h2_on_data_chunk_recv);
  nghttp2_session_callbacks_set_on_stream_close_callback(callbacks,
                                                         safe_h2_on_stream_close);

  rv = nghttp2_session_client_new(&session, callbacks, &bridge);
  if(rv != 0) {
    safe_h2_set_error(&bridge, nghttp2_strerror(rv));
    nghttp2_session_callbacks_del(callbacks);
    return -1;
  }

  nva = calloc(header_count, sizeof(*nva));
  if(!nva) {
    safe_h2_set_error(&bridge, "out of memory building HTTP/2 request headers");
    nghttp2_session_del(session);
    nghttp2_session_callbacks_del(callbacks);
    return -1;
  }
  for(i = 0; i < header_count; ++i) {
    nva[i].name = (uint8_t *)headers[i].name;
    nva[i].namelen = headers[i].namelen;
    nva[i].value = (uint8_t *)headers[i].value;
    nva[i].valuelen = headers[i].valuelen;
    nva[i].flags = headers[i].flags;
  }

  settings[0].settings_id = NGHTTP2_SETTINGS_ENABLE_PUSH;
  settings[0].value = allow_push ? 1 : 0;
  rv = nghttp2_submit_settings(session, NGHTTP2_FLAG_NONE, settings, 1);
  if(rv != 0) {
    safe_h2_set_error(&bridge, nghttp2_strerror(rv));
    free(nva);
    nghttp2_session_del(session);
    nghttp2_session_callbacks_del(callbacks);
    return -1;
  }

  memset(&data_prd, 0, sizeof(data_prd));
  if(has_body) {
    data_prd.source.ptr = NULL;
    data_prd.read_callback = safe_h2_body_read_callback;
  }
  stream_id =
      nghttp2_submit_request(session, NULL, nva, header_count, has_body ? &data_prd : NULL, NULL);
  free(nva);
  if(stream_id < 0) {
    safe_h2_set_error(&bridge, nghttp2_strerror(stream_id));
    nghttp2_session_del(session);
    nghttp2_session_callbacks_del(callbacks);
    return -1;
  }
  bridge.request_stream_id = stream_id;

  while(!bridge.stream_closed) {
    rv = nghttp2_session_send(session);
    if(rv == NGHTTP2_ERR_WOULDBLOCK) {
      continue;
    }
    if(rv != 0) {
      safe_h2_set_error_if_empty(&bridge, nghttp2_strerror(rv));
      nghttp2_session_del(session);
      nghttp2_session_callbacks_del(callbacks);
      free(bridge.header_block);
      free(bridge.push_block);
      return -1;
    }
    if(bridge.stream_closed) {
      break;
    }
    rv = nghttp2_session_recv(session);
    if(rv == NGHTTP2_ERR_WOULDBLOCK) {
      continue;
    }
    if(rv == NGHTTP2_ERR_EOF) {
      safe_h2_set_error_if_empty(&bridge, "unexpected EOF in HTTP/2 stream");
      nghttp2_session_del(session);
      nghttp2_session_callbacks_del(callbacks);
      free(bridge.header_block);
      free(bridge.push_block);
      return -1;
    }
    if(rv != 0) {
      safe_h2_set_error_if_empty(&bridge, nghttp2_strerror(rv));
      nghttp2_session_del(session);
      nghttp2_session_callbacks_del(callbacks);
      free(bridge.header_block);
      free(bridge.push_block);
      return -1;
    }
  }

  rv = nghttp2_session_send(session);
  if(rv != 0) {
    safe_h2_set_error_if_empty(&bridge, nghttp2_strerror(rv));
    nghttp2_session_del(session);
    nghttp2_session_callbacks_del(callbacks);
    free(bridge.header_block);
    free(bridge.push_block);
    return -1;
  }

  if(stream_error_code) {
    *stream_error_code = bridge.stream_error_code;
  }
  nghttp2_session_del(session);
  nghttp2_session_callbacks_del(callbacks);
  free(bridge.header_block);
  free(bridge.push_block);
  return 0;
}
