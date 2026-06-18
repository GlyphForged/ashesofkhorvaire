#!/bin/sh
set -eu
umask 077

DB=${ASHES_DB_PATH:-/var/lib/ashes-wiki/ashes.db}
BACKUP_DIR=${1:-/var/backups/ashes-wiki}
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
PLAIN="$BACKUP_DIR/ashes-$STAMP.db"

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required for online backups." >&2
  exit 1
fi
if [ ! -f "$DB" ]; then
  echo "Database not found: $DB" >&2
  exit 1
fi

install -d -m 0700 "$BACKUP_DIR"
sqlite3 "$DB" ".backup '$PLAIN'"
gzip "$PLAIN"
echo "$PLAIN.gz"
