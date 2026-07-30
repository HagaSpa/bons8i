#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../infra/cloudflare"

# cloudflare provider には credentials ファイルの概念が無く、環境変数か HCL 直書きの二択。
# 未設定のまま実行すると 400 "Missing X-Auth-Key, X-Auth-Email or Authorization headers" になる
export CLOUDFLARE_API_TOKEN
CLOUDFLARE_API_TOKEN=$(aws ssm get-parameter \
  --name /bons8i/cloudflare/terraform/api-token \
  --with-decryption --query Parameter.Value --output text)

exec terraform "$@"
