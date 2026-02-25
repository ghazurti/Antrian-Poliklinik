#!/bin/bash
# start.sh — Jalankan server Antrian Poli
# Usage: ./start.sh

cd "$(dirname "$0")"

# Bebaskan port jika ada proses lama
PORT=$(grep '^PORT=' .env 2>/dev/null | cut -d= -f2)
PORT=${PORT:-3030}

if lsof -ti:$PORT &>/dev/null; then
    echo "⚠️  Port $PORT sedang dipakai, mematikan proses lama..."
    lsof -ti:$PORT | xargs kill -9
    sleep 1
fi

echo "🚀 Menjalankan Antrian Poli di http://localhost:$PORT"
cargo run
