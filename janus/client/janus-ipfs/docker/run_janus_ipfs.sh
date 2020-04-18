#!/usr/bin/env bash
# This script is intented to be used as an ENTRYPOINT in Docker container

set -euo pipefail
set -m # enable Bash job management to pass signals to children

BASH=$(command -v bash)

declare -p IPFS_MULTIADDR &>/dev/null || {
    echo >&2 "IPFS_MULTIADDR should be passed to container as an environment variable"
    exit 1
}

JANUS_PORT=7777

# Search for TCP port ('-t') in arguments, and take next argument
ARGS="$*"
FOUND=0
for arg in $ARGS; do
  if [ "$FOUND" = 1 ]; then
    JANUS_PORT=$arg
    break
  fi
  if [ "$arg" = "-t" ]; then
    FOUND=1
  fi
done

# Run Server & Janus-IPFS in parallel, fail if any of the processes fails
$BASH wait.sh \
  "./janus-server $*" \
  "./janus-ipfs /ip4/127.0.0.1/tcp/$JANUS_PORT $IPFS_MULTIADDR"
