#!/bin/sh
set -eu
dir=${1:-/var/lib/pet}
umask 077
mkdir -p "$dir"
if [ -e "$dir/server.key" ]; then
    echo "$dir/server.key exists, refusing to overwrite" >&2
    exit 1
fi
openssl genpkey -algorithm X25519 -out "$dir/server.pem"
openssl pkey -in "$dir/server.pem" -outform DER | tail -c 32 | od -An -tx1 | tr -d ' \n' > "$dir/server.key"
openssl pkey -in "$dir/server.pem" -pubout -outform DER | tail -c 32 | od -An -tx1 | tr -d ' \n' > "$dir/server.pub"
echo "public key: $(cat "$dir/server.pub")"
