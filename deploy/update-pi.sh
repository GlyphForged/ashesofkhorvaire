#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PACK=${ASHES_CONTENT_PACK:-$ROOT/content/campaign.json}
FORCE=

if [ "${1:-}" = "--force" ]; then
  FORCE=--force
elif [ "$#" -gt 0 ]; then
  echo "Usage: ./deploy/update-pi.sh [--force]" >&2
  exit 2
fi

if [ ! -f "$PACK" ]; then
  echo "Content pack not found: $PACK" >&2
  exit 1
fi

cd "$ROOT"
sh ./deploy/build-release.sh
sudo ashes-wiki-backup
sudo systemctl stop ashes-wiki.service

cleanup() {
  sudo systemctl start ashes-wiki.service
}
trap cleanup EXIT INT TERM

sudo env ASHES_SKIP_RESTART=1 sh ./deploy/install.sh
sudo install -o root -g root -m 0644 "$PACK" /opt/ashes-wiki/content/campaign.json
sudo -u ashes-wiki /opt/ashes-wiki/bin/ashes-content apply \
  /opt/ashes-wiki/content/campaign.json $FORCE \
  --database-url sqlite:///var/lib/ashes-wiki/ashes.db

sudo systemctl start ashes-wiki.service
trap - EXIT INT TERM
attempt=0
until curl --fail --silent --show-error http://127.0.0.1:3000/healthz; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 10 ]; then
    echo "Wiki did not become healthy after the update." >&2
    exit 1
  fi
  sleep 1
done
echo "Orange Pi application and campaign content are current."
