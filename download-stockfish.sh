#!/usr/bin/env bash
set -euo pipefail

url="https://github.com/official-stockfish/Stockfish/releases/download/sf_18/stockfish-ubuntu-x86-64.tar"
sha256="5c6f38b02a4da5f3ffe763f27da6c3e743eebefd92b50cb3661623b96696adff"
work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT

curl --location --fail --show-error "${url}" --output "${work_dir}/stockfish.tar"
printf '%s  %s\n' "${sha256}" "${work_dir}/stockfish.tar" | sha256sum --check -
tar -xf "${work_dir}/stockfish.tar" -C "${work_dir}" \
  stockfish/stockfish-ubuntu-x86-64
install -m0755 "${work_dir}/stockfish/stockfish-ubuntu-x86-64" assets/stockfish
