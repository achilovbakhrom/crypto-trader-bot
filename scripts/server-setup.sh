#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# Server bootstrap script — run ONCE on a fresh Hetzner Ubuntu 24.04 VPS.
#
# Usage:
#   ssh root@<your-server-ip>
#   curl -sO https://raw.githubusercontent.com/achilovbakhrom/crypto-trader-bot/main/scripts/server-setup.sh
#   chmod +x server-setup.sh && ./server-setup.sh
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

DEPLOY_DIR="/opt/crypto-trader-bot"
DEPLOY_USER="trader"

echo "=== Crypto Trader Bot — Server Setup ==="

# ── System update ──────────────────────────────────────────────────────────────
echo "[1/6] Updating system..."
apt-get update -qq && apt-get upgrade -y -qq

# ── Install Docker ─────────────────────────────────────────────────────────────
echo "[2/6] Installing Docker..."
apt-get install -y -qq ca-certificates curl gnupg lsb-release ufw

install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | \
  gpg --dearmor -o /etc/apt/keyrings/docker.gpg
chmod a+r /etc/apt/keyrings/docker.gpg

echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
  https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" \
  > /etc/apt/sources.list.d/docker.list

apt-get update -qq
apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

systemctl enable docker
systemctl start docker

# ── Create deploy user ────────────────────────────────────────────────────────
echo "[3/6] Creating deploy user..."
if ! id "$DEPLOY_USER" &>/dev/null; then
  useradd -m -s /bin/bash "$DEPLOY_USER"
  usermod -aG docker "$DEPLOY_USER"
fi

# ── Create deploy directory ───────────────────────────────────────────────────
echo "[4/6] Creating deploy directory..."
mkdir -p "$DEPLOY_DIR"
chown -R "$DEPLOY_USER:$DEPLOY_USER" "$DEPLOY_DIR"

# ── Firewall ──────────────────────────────────────────────────────────────────
echo "[5/6] Configuring firewall..."
ufw --force reset
ufw default deny incoming
ufw default allow outgoing
ufw allow ssh
ufw allow 3000/tcp   # web API
ufw allow 5080/tcp   # OpenObserve UI (restrict to your IP in prod)
ufw --force enable

# ── Copy docker-compose.prod.yml ─────────────────────────────────────────────
echo "[6/6] Setup complete."
echo ""
echo "Next steps:"
echo "  1. Copy docker-compose.prod.yml to $DEPLOY_DIR/"
echo "     scp docker-compose.prod.yml root@<server>:$DEPLOY_DIR/"
echo ""
echo "  2. Create .env on the server:"
echo "     ssh root@<server>"
echo "     cd $DEPLOY_DIR && nano .env"
echo "     (fill in all production values)"
echo ""
echo "  3. Add GitHub Actions secrets:"
echo "     HETZNER_HOST     = <server-ip>"
echo "     HETZNER_USER     = root  (or $DEPLOY_USER)"
echo "     HETZNER_SSH_KEY  = <your-private-ssh-key>"
echo "     DEPLOY_DIR       = $DEPLOY_DIR"
echo ""
echo "  4. Push to main — CI/CD deploys automatically."
echo ""
echo "Server is ready."
