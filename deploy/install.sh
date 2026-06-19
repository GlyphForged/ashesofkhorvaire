#!/bin/sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
  echo "Run with sudo: sudo ./deploy/install.sh" >&2
  exit 1
fi

SOURCE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BINARY="$SOURCE_DIR/target/release/ashes-wiki"
CONTENT_BINARY="$SOURCE_DIR/target/release/ashes-content"

if [ ! -x "$BINARY" ]; then
  echo "Missing release binary. Run ./deploy/build-release.sh as your normal user first." >&2
  exit 1
fi
if [ ! -x "$CONTENT_BINARY" ]; then
  echo "Missing content updater. Run ./deploy/build-release.sh as your normal user first." >&2
  exit 1
fi

if ! getent group ashes-wiki >/dev/null 2>&1; then
  groupadd --system ashes-wiki
fi
if ! id ashes-wiki >/dev/null 2>&1; then
  useradd --system --gid ashes-wiki --home-dir /var/lib/ashes-wiki --shell /usr/sbin/nologin ashes-wiki
fi

install -d -o root -g root -m 0755 /opt/ashes-wiki/bin /opt/ashes-wiki/static /opt/ashes-wiki/content
install -d -o ashes-wiki -g ashes-wiki -m 0750 /var/lib/ashes-wiki
install -d -o root -g ashes-wiki -m 0750 /etc/ashes-wiki
install -o root -g root -m 0755 "$BINARY" /opt/ashes-wiki/bin/ashes-wiki
install -o root -g root -m 0755 "$CONTENT_BINARY" /opt/ashes-wiki/bin/ashes-content
if [ -f "$SOURCE_DIR/content/campaign.json" ]; then
  install -o root -g root -m 0644 "$SOURCE_DIR/content/campaign.json" /opt/ashes-wiki/content/campaign.json
fi

rm -rf /opt/ashes-wiki/static
cp -R "$SOURCE_DIR/static" /opt/ashes-wiki/static
chown -R root:root /opt/ashes-wiki/static
find /opt/ashes-wiki/static -type d -exec chmod 0755 {} \;
find /opt/ashes-wiki/static -type f -exec chmod 0644 {} \;

if [ ! -f /etc/ashes-wiki/ashes-wiki.env ]; then
  install -o root -g ashes-wiki -m 0640 "$SOURCE_DIR/deploy/ashes-wiki.env.example" /etc/ashes-wiki/ashes-wiki.env
fi

install -o root -g root -m 0644 "$SOURCE_DIR/deploy/ashes-wiki.service" /etc/systemd/system/ashes-wiki.service
install -o root -g root -m 0755 "$SOURCE_DIR/deploy/backup.sh" /usr/local/sbin/ashes-wiki-backup

systemctl daemon-reload
systemctl enable ashes-wiki.service
if [ "${ASHES_SKIP_RESTART:-0}" != "1" ]; then
  systemctl restart ashes-wiki.service
  systemctl --no-pager --full status ashes-wiki.service
fi

echo "Ashes Wiki is listening locally on 127.0.0.1:3000. Configure Caddy before exposing it."
