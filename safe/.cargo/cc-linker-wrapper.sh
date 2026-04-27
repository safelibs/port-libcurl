#!/usr/bin/env bash
set -euo pipefail

real_cc="${PORT_LIBCURL_REAL_CC:-cc}"
needs_public_abi_filter=0

for arg in "$@"; do
  case "$arg" in
    *libcurl-openssl.map|*libcurl-gnutls.map)
      needs_public_abi_filter=1
      break
      ;;
  esac
done

if [[ "${needs_public_abi_filter}" -eq 0 ]]; then
  exec "${real_cc}" "$@"
fi

filtered=()
for arg in "$@"; do
  case "$arg" in
    -Wl,--version-script=*rustc*/list)
      continue
      ;;
  esac
  filtered+=("${arg}")
done

exec "${real_cc}" "${filtered[@]}"
