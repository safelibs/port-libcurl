#include <curl/curl.h>

#include <errno.h>
#include <getopt.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

struct options {
  const char *scenario;
  const char *implementation;
  const char *flavor;
  const char *url;
  const char *resolve_host;
  const char *output_path;
  long transfers;
  long parallel;
  int samples;
  int warmups;
  long http_version;
  long max_host_connections;
  int insecure;
  int pipewait;
  int share_ssl_session;
  int fresh_connect;
  int forbid_reuse;
};

struct transfer_slot {
  CURL *easy;
  uint64_t bytes;
};

static void usage(FILE *stream)
{
  fprintf(
    stream,
    "usage: multi_parallel --scenario <id> --implementation <name> --flavor <name> "
    "--url <url> --transfers <n> --parallel <n> --samples <n> --warmups <n> "
    "--output <path> [--http-version <1.1|2|default>] [--insecure] "
    "[--resolve-host <host:port:addr>] [--pipewait] [--share-ssl-session] "
    "[--fresh-connect] [--forbid-reuse] [--max-host-connections <n>]\n"
  );
}

static int cmp_double(const void *lhs, const void *rhs)
{
  const double left = *(const double *)lhs;
  const double right = *(const double *)rhs;
  if(left < right)
    return -1;
  if(left > right)
    return 1;
  return 0;
}

static uint64_t now_ns(void)
{
  struct timespec ts;
  if(clock_gettime(CLOCK_MONOTONIC, &ts) != 0) {
    fprintf(stderr, "clock_gettime failed: %s\n", strerror(errno));
    exit(1);
  }
  return ((uint64_t)ts.tv_sec * 1000000000ULL) + (uint64_t)ts.tv_nsec;
}

static size_t discard_body(char *ptr, size_t size, size_t nmemb, void *userdata)
{
  struct transfer_slot *slot = (struct transfer_slot *)userdata;
  const size_t total = size * nmemb;
  (void)ptr;
  slot->bytes += (uint64_t)total;
  return total;
}

static long parse_http_version(const char *raw)
{
  if(strcmp(raw, "default") == 0)
    return CURL_HTTP_VERSION_NONE;
  if(strcmp(raw, "1.1") == 0)
    return CURL_HTTP_VERSION_1_1;
  if(strcmp(raw, "2") == 0)
    return CURL_HTTP_VERSION_2_0;
  fprintf(stderr, "unsupported --http-version value: %s\n", raw);
  exit(2);
}

static void require_option(const char *value, const char *flag)
{
  if(value == NULL || *value == '\0') {
    fprintf(stderr, "missing required option %s\n", flag);
    usage(stderr);
    exit(2);
  }
}

static void json_write_string(FILE *stream, const char *value)
{
  const unsigned char *cursor = (const unsigned char *)value;
  fputc('"', stream);
  while(*cursor) {
    switch(*cursor) {
    case '\"':
      fputs("\\\"", stream);
      break;
    case '\\':
      fputs("\\\\", stream);
      break;
    case '\b':
      fputs("\\b", stream);
      break;
    case '\f':
      fputs("\\f", stream);
      break;
    case '\n':
      fputs("\\n", stream);
      break;
    case '\r':
      fputs("\\r", stream);
      break;
    case '\t':
      fputs("\\t", stream);
      break;
    default:
      if(*cursor < 0x20)
        fprintf(stream, "\\u%04x", *cursor);
      else
        fputc(*cursor, stream);
      break;
    }
    ++cursor;
  }
  fputc('"', stream);
}

static void write_json(
  const struct options *opts,
  const double *samples_ms,
  uint64_t total_bytes,
  double median_ms,
  double min_ms,
  double max_ms
)
{
  FILE *output = fopen(opts->output_path, "w");
  int i;

  if(output == NULL) {
    fprintf(stderr, "failed to open %s: %s\n", opts->output_path, strerror(errno));
    exit(1);
  }

  fprintf(output, "{\n");
  fprintf(output, "  \"schema_version\": 1,\n");
  fprintf(output, "  \"scenario_id\": ");
  json_write_string(output, opts->scenario);
  fprintf(output, ",\n  \"harness\": \"multi_parallel\",\n");
  fprintf(output, "  \"implementation\": ");
  json_write_string(output, opts->implementation);
  fprintf(output, ",\n  \"flavor\": ");
  json_write_string(output, opts->flavor);
  fprintf(output, ",\n  \"url\": ");
  json_write_string(output, opts->url);
  fprintf(output, ",\n  \"http_version\": ");
  if(opts->http_version == CURL_HTTP_VERSION_1_1)
    json_write_string(output, "1.1");
  else if(opts->http_version == CURL_HTTP_VERSION_2_0)
    json_write_string(output, "2");
  else
    json_write_string(output, "default");
  fprintf(output, ",\n  \"run_count\": %d,\n", opts->samples);
  fprintf(output, "  \"warmup_count\": %d,\n", opts->warmups);
  fprintf(output, "  \"transfers_per_run\": %ld,\n", opts->transfers);
  fprintf(output, "  \"parallelism\": %ld,\n", opts->parallel);
  fprintf(output, "  \"bytes_transferred\": %" PRIu64 ",\n", total_bytes);
  fprintf(output, "  \"bytes_per_run\": %.0f,\n", (double)total_bytes / (double)opts->samples);
  fprintf(output, "  \"share_ssl_session\": %s,\n", opts->share_ssl_session ? "true" : "false");
  fprintf(output, "  \"fresh_connect\": %s,\n", opts->fresh_connect ? "true" : "false");
  fprintf(output, "  \"forbid_reuse\": %s,\n", opts->forbid_reuse ? "true" : "false");
  fprintf(output, "  \"max_host_connections\": %ld,\n", opts->max_host_connections);
  fprintf(output, "  \"median_wall_time_ms\": %.3f,\n", median_ms);
  fprintf(output, "  \"min_wall_time_ms\": %.3f,\n", min_ms);
  fprintf(output, "  \"max_wall_time_ms\": %.3f,\n", max_ms);
  fprintf(output, "  \"sample_wall_time_ms\": [");
  for(i = 0; i < opts->samples; ++i) {
    if(i)
      fputs(", ", output);
    fprintf(output, "%.3f", samples_ms[i]);
  }
  fprintf(output, "]\n}\n");
  fclose(output);
}

static void apply_transfer_options(
  CURL *easy,
  const struct options *opts,
  struct curl_slist *resolve,
  struct transfer_slot *slot,
  CURLSH *share
)
{
  curl_easy_setopt(easy, CURLOPT_URL, opts->url);
  curl_easy_setopt(easy, CURLOPT_NOSIGNAL, 1L);
  curl_easy_setopt(easy, CURLOPT_WRITEFUNCTION, discard_body);
  curl_easy_setopt(easy, CURLOPT_WRITEDATA, slot);
  curl_easy_setopt(easy, CURLOPT_PRIVATE, slot);
  curl_easy_setopt(easy, CURLOPT_HTTPGET, 1L);
  curl_easy_setopt(easy, CURLOPT_HTTP_VERSION, opts->http_version);
  if(opts->pipewait)
    curl_easy_setopt(easy, CURLOPT_PIPEWAIT, 1L);
  if(opts->insecure) {
    curl_easy_setopt(easy, CURLOPT_SSL_VERIFYPEER, 0L);
    curl_easy_setopt(easy, CURLOPT_SSL_VERIFYHOST, 0L);
  }
  if(opts->fresh_connect)
    curl_easy_setopt(easy, CURLOPT_FRESH_CONNECT, 1L);
  if(opts->forbid_reuse)
    curl_easy_setopt(easy, CURLOPT_FORBID_REUSE, 1L);
  if(resolve != NULL)
    curl_easy_setopt(easy, CURLOPT_RESOLVE, resolve);
  if(share != NULL)
    curl_easy_setopt(easy, CURLOPT_SHARE, share);
}

static void start_transfer(
  CURLM *multi,
  const struct options *opts,
  struct curl_slist *resolve,
  struct transfer_slot *slot,
  CURLSH *share
)
{
  CURLMcode mc;

  slot->bytes = 0;
  slot->easy = curl_easy_init();
  if(slot->easy == NULL) {
    fprintf(stderr, "curl_easy_init failed\n");
    exit(1);
  }
  apply_transfer_options(slot->easy, opts, resolve, slot, share);
  mc = curl_multi_add_handle(multi, slot->easy);
  if(mc != CURLM_OK) {
    fprintf(stderr, "curl_multi_add_handle failed: %s\n", curl_multi_strerror(mc));
    exit(1);
  }
}

static void run_sample(const struct options *opts, uint64_t *bytes_out, double *elapsed_ms_out)
{
  CURLM *multi;
  CURLSH *share = NULL;
  struct curl_slist *resolve = NULL;
  struct transfer_slot *slots;
  uint64_t start_ns;
  uint64_t stop_ns;
  uint64_t bytes = 0;
  long started = 0;
  long completed = 0;
  long active_handles = 0;
  long slot_index;

  slots = calloc((size_t)opts->parallel, sizeof(*slots));
  if(slots == NULL) {
    fprintf(stderr, "out of memory\n");
    exit(1);
  }

  if(opts->resolve_host != NULL) {
    resolve = curl_slist_append(resolve, opts->resolve_host);
    if(resolve == NULL) {
      fprintf(stderr, "curl_slist_append failed for %s\n", opts->resolve_host);
      exit(1);
    }
  }

  multi = curl_multi_init();
  if(multi == NULL) {
    fprintf(stderr, "curl_multi_init failed\n");
    exit(1);
  }

#ifdef CURLPIPE_MULTIPLEX
  curl_multi_setopt(multi, CURLMOPT_PIPELINING, CURLPIPE_MULTIPLEX);
#endif
  if(opts->max_host_connections > 0)
    curl_multi_setopt(multi, CURLMOPT_MAX_HOST_CONNECTIONS, opts->max_host_connections);

  if(opts->share_ssl_session) {
    share = curl_share_init();
    if(share == NULL) {
      fprintf(stderr, "curl_share_init failed\n");
      exit(1);
    }
    curl_share_setopt(share, CURLSHOPT_SHARE, CURL_LOCK_DATA_SSL_SESSION);
  }

  for(slot_index = 0; slot_index < opts->parallel && started < opts->transfers; ++slot_index) {
    start_transfer(multi, opts, resolve, &slots[slot_index], share);
    ++started;
    ++active_handles;
  }

  start_ns = now_ns();
  while(completed < opts->transfers) {
    int still_running = 0;
    int msgs_in_queue = 0;
    int numfds = 0;
    long poll_timeout_ms = 10;
    CURLMcode mc;

    mc = curl_multi_perform(multi, &still_running);
    if(mc != CURLM_OK) {
      fprintf(stderr, "curl_multi_perform failed: %s\n", curl_multi_strerror(mc));
      exit(1);
    }

    for(;;) {
      CURLMsg *msg = curl_multi_info_read(multi, &msgs_in_queue);
      struct transfer_slot *slot = NULL;
      long response_code = 0;
      if(msg == NULL)
        break;
      if(msg->msg != CURLMSG_DONE)
        continue;

      if(curl_easy_getinfo(msg->easy_handle, CURLINFO_PRIVATE, &slot) != CURLE_OK || slot == NULL) {
        fprintf(stderr, "curl_easy_getinfo(CURLINFO_PRIVATE) failed\n");
        exit(1);
      }
      if(msg->data.result != CURLE_OK) {
        fprintf(stderr, "transfer failed: %s\n", curl_easy_strerror(msg->data.result));
        exit(1);
      }
      if(curl_easy_getinfo(msg->easy_handle, CURLINFO_RESPONSE_CODE, &response_code) != CURLE_OK) {
        fprintf(stderr, "curl_easy_getinfo(CURLINFO_RESPONSE_CODE) failed\n");
        exit(1);
      }
      if((response_code / 100L) != 2L) {
        fprintf(stderr, "unexpected HTTP response code: %ld\n", response_code);
        exit(1);
      }

      bytes += slot->bytes;
      curl_multi_remove_handle(multi, msg->easy_handle);
      curl_easy_cleanup(msg->easy_handle);
      slot->easy = NULL;
      slot->bytes = 0;
      ++completed;
      --active_handles;

      if(started < opts->transfers) {
        start_transfer(multi, opts, resolve, slot, share);
        ++started;
        ++active_handles;
      }
    }

    if(completed >= opts->transfers)
      break;

    if(active_handles <= 0)
      continue;

    mc = curl_multi_timeout(multi, &poll_timeout_ms);
    if(mc != CURLM_OK) {
      fprintf(stderr, "curl_multi_timeout failed: %s\n", curl_multi_strerror(mc));
      exit(1);
    }
    if(poll_timeout_ms < 0 || poll_timeout_ms > 10)
      poll_timeout_ms = 10;

    mc = curl_multi_poll(multi, NULL, 0, (int)poll_timeout_ms, &numfds);
    if(mc != CURLM_OK) {
      fprintf(stderr, "curl_multi_poll failed: %s\n", curl_multi_strerror(mc));
      exit(1);
    }
  }
  stop_ns = now_ns();

  for(slot_index = 0; slot_index < opts->parallel; ++slot_index) {
    if(slots[slot_index].easy != NULL) {
      curl_multi_remove_handle(multi, slots[slot_index].easy);
      curl_easy_cleanup(slots[slot_index].easy);
    }
  }
  if(share != NULL)
    curl_share_cleanup(share);
  curl_slist_free_all(resolve);
  curl_multi_cleanup(multi);
  free(slots);

  *bytes_out = bytes;
  *elapsed_ms_out = (double)(stop_ns - start_ns) / 1000000.0;
}

int main(int argc, char **argv)
{
  static const struct option long_options[] = {
    {"scenario", required_argument, NULL, 'S'},
    {"implementation", required_argument, NULL, 'I'},
    {"flavor", required_argument, NULL, 'F'},
    {"url", required_argument, NULL, 'u'},
    {"transfers", required_argument, NULL, 'n'},
    {"parallel", required_argument, NULL, 'p'},
    {"samples", required_argument, NULL, 's'},
    {"warmups", required_argument, NULL, 'w'},
    {"output", required_argument, NULL, 'o'},
    {"http-version", required_argument, NULL, 'H'},
    {"resolve-host", required_argument, NULL, 'r'},
    {"max-host-connections", required_argument, NULL, 'm'},
    {"insecure", no_argument, NULL, 'k'},
    {"pipewait", no_argument, NULL, 'P'},
    {"share-ssl-session", no_argument, NULL, 'c'},
    {"fresh-connect", no_argument, NULL, 'f'},
    {"forbid-reuse", no_argument, NULL, 'R'},
    {"help", no_argument, NULL, 'h'},
    {NULL, 0, NULL, 0}
  };
  struct options opts;
  double *samples_ms = NULL;
  double *sorted_ms = NULL;
  double median_ms;
  double min_ms;
  double max_ms;
  uint64_t total_bytes = 0;
  int ch;
  int i;

  memset(&opts, 0, sizeof(opts));
  opts.http_version = CURL_HTTP_VERSION_NONE;

  while((ch = getopt_long(argc, argv, "", long_options, NULL)) != -1) {
    switch(ch) {
    case 'S':
      opts.scenario = optarg;
      break;
    case 'I':
      opts.implementation = optarg;
      break;
    case 'F':
      opts.flavor = optarg;
      break;
    case 'u':
      opts.url = optarg;
      break;
    case 'n':
      opts.transfers = strtol(optarg, NULL, 10);
      break;
    case 'p':
      opts.parallel = strtol(optarg, NULL, 10);
      break;
    case 's':
      opts.samples = (int)strtol(optarg, NULL, 10);
      break;
    case 'w':
      opts.warmups = (int)strtol(optarg, NULL, 10);
      break;
    case 'o':
      opts.output_path = optarg;
      break;
    case 'H':
      opts.http_version = parse_http_version(optarg);
      break;
    case 'r':
      opts.resolve_host = optarg;
      break;
    case 'm':
      opts.max_host_connections = strtol(optarg, NULL, 10);
      break;
    case 'k':
      opts.insecure = 1;
      break;
    case 'P':
      opts.pipewait = 1;
      break;
    case 'c':
      opts.share_ssl_session = 1;
      break;
    case 'f':
      opts.fresh_connect = 1;
      break;
    case 'R':
      opts.forbid_reuse = 1;
      break;
    case 'h':
      usage(stdout);
      return 0;
    default:
      usage(stderr);
      return 2;
    }
  }

  require_option(opts.scenario, "--scenario");
  require_option(opts.implementation, "--implementation");
  require_option(opts.flavor, "--flavor");
  require_option(opts.url, "--url");
  require_option(opts.output_path, "--output");
  if(opts.transfers <= 0 || opts.parallel <= 0 || opts.samples <= 0 || opts.warmups < 0) {
    fprintf(stderr, "invalid loop counts\n");
    return 2;
  }
  if(opts.parallel > opts.transfers)
    opts.parallel = opts.transfers;

  if(curl_global_init(CURL_GLOBAL_DEFAULT) != CURLE_OK) {
    fprintf(stderr, "curl_global_init failed\n");
    return 1;
  }

  samples_ms = calloc((size_t)opts.samples, sizeof(*samples_ms));
  sorted_ms = calloc((size_t)opts.samples, sizeof(*sorted_ms));
  if(samples_ms == NULL || sorted_ms == NULL) {
    fprintf(stderr, "out of memory\n");
    return 1;
  }

  for(i = 0; i < opts.warmups; ++i) {
    uint64_t warmup_bytes = 0;
    double warmup_ms = 0.0;
    run_sample(&opts, &warmup_bytes, &warmup_ms);
  }

  for(i = 0; i < opts.samples; ++i) {
    uint64_t sample_bytes = 0;
    run_sample(&opts, &sample_bytes, &samples_ms[i]);
    total_bytes += sample_bytes;
    sorted_ms[i] = samples_ms[i];
  }

  qsort(sorted_ms, (size_t)opts.samples, sizeof(*sorted_ms), cmp_double);
  min_ms = sorted_ms[0];
  max_ms = sorted_ms[opts.samples - 1];
  if((opts.samples % 2) == 0) {
    median_ms = (sorted_ms[(opts.samples / 2) - 1] + sorted_ms[opts.samples / 2]) / 2.0;
  }
  else {
    median_ms = sorted_ms[opts.samples / 2];
  }

  write_json(&opts, samples_ms, total_bytes, median_ms, min_ms, max_ms);

  free(sorted_ms);
  free(samples_ms);
  curl_global_cleanup();
  return 0;
}
