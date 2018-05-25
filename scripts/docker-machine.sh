#!/bin/bash

set -o errexit
cd "$(dirname "${BASH_SOURCE}")"

docker-machine create --driver google \
  --google-project media-9x16 \
  --google-zone us-central1-a \
  --google-machine-type n1-highcpu-16 \
  --google-disk-size 100 \
  stream

echo $(docker-machine env stream)
