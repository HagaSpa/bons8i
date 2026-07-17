#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BUILD_DIR="infra/aws/.build"
mkdir -p "$BUILD_DIR"

zip -j -X "$BUILD_DIR/external-probe.zip" infra/aws/lambda/external-probe/index.mjs

aws lambda update-function-code \
  --function-name bons8i-external-probe \
  --zip-file "fileb://$BUILD_DIR/external-probe.zip" \
  --no-cli-pager
