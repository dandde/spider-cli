#!/bin/bash

# spider-cli Automation Script
# Handles: Testing -> Build -> Launch -> Crawl

set -e # Exit on error

echo "🚀 Starting spider-cli Automation Workflow..."

# 1. Run Tests
echo "🧪 Running unit and integration tests..."
cargo test --quiet

# 2. Build Release
echo "🏗️  Building release binary..."
cargo build --release --quiet

# 3. Up the Server & Perform Crawl Task
echo "🌐 Launching dashboard and starting crawl task on https://quotes.toscrape.com..."
echo "📊 Dashboard will be available at http://localhost:3030"

# Note: The server and crawler run together when the --dashboard flag is used.
./target/release/spider-cli crawl https://quotes.toscrape.com --dashboard
