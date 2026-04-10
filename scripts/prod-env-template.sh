#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# Production .env template — run on server to create /opt/crypto-trader-bot/.env
#
# Usage: ./scripts/prod-env-template.sh > /opt/crypto-trader-bot/.env
# Then edit the file with real values.
# ─────────────────────────────────────────────────────────────────────────────

cat << 'EOF'
# ─────────────────────────────────────────────────────────────────
# Production environment — Hetzner Singapore
# Created by prod-env-template.sh
# ─────────────────────────────────────────────────────────────────

# ── Chain (Base mainnet) ───────────────────────────────────────────
CHAIN_RPC_URL=https://base-mainnet.g.alchemy.com/v2/YOUR_KEY_HERE
CHAIN_WS_URL=wss://base-mainnet.g.alchemy.com/v2/YOUR_KEY_HERE

# ── Binance API ────────────────────────────────────────────────────
BINANCE_API_KEY=your_binance_api_key
BINANCE_API_SECRET=your_binance_api_secret

# ── Ethereum wallet (mainnet — keep this safe) ─────────────────────
PRIVATE_KEY=0xyour_mainnet_private_key

# ── Database ──────────────────────────────────────────────────────
DATABASE_URL=postgres://trader:CHANGE_ME@postgres:5432/crypto_trader

POSTGRES_DB=crypto_trader
POSTGRES_USER=trader
POSTGRES_PASSWORD=CHANGE_ME_USE_STRONG_PASSWORD

# ── OpenObserve ───────────────────────────────────────────────────
OPENOBSERVE_URL=http://openobserve:5080
OPENOBSERVE_TOKEN=
OPENOBSERVE_EMAIL=admin@yourdomain.com
OPENOBSERVE_PASSWORD=CHANGE_ME_USE_STRONG_PASSWORD

# ── App ───────────────────────────────────────────────────────────
APP_ENV=production
RUST_LOG=info
IMAGE_TAG=latest
EOF
