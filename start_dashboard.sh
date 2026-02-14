#!/bin/bash

# start_dashboard.sh
# Launches the spider-cli monitoring dashboard server.

set -e

# Default port
PORT=${1:-3030}

echo "🔍 Checking if port $PORT is available..."
if lsof -Pi :$PORT -sTCP:LISTEN -t >/dev/null ; then
    echo "⚡ Port $PORT is in use. Killing existing process..."
    lsof -ti:$PORT | xargs kill -9
    sleep 1
fi

echo "🏗️  Ensuring release binary is built..."
cargo build --release --quiet

echo "🌐 Starting spider-cli dashboard on http://localhost:$PORT..."
./target/release/spider-cli serve --port $PORT
