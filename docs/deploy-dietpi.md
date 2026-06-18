# Deploy on an Orange Pi running DietPi

This path targets a Debian-based DietPi installation on either 64-bit or 32-bit ARM. The release is built natively on the board, avoiding cross-compilation and Orange Pi model assumptions.

## 1. Confirm the board

```sh
uname -m
cat /etc/os-release
```

`aarch64`/`arm64` is preferred. The scripts also accept `armv7l` and `armv8l`.

## 2. Install build and runtime packages

```sh
sudo apt update
sudo apt install -y build-essential curl pkg-config sqlite3 ca-certificates
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
```

Clone or copy the repository to the Orange Pi, then build as your normal user:

```sh
./deploy/build-release.sh
```

The first build can take several minutes on a small board. Later deployments reuse Cargo's cache.

## 3. Install the service

```sh
sudo ./deploy/install.sh
curl --fail --show-error http://127.0.0.1:3000/healthz
```

The installer creates:

- `/opt/ashes-wiki` for the binary and static assets;
- `/var/lib/ashes-wiki` for the SQLite database;
- `/etc/ashes-wiki/ashes-wiki.env` for configuration;
- a locked-down `ashes-wiki` system user and systemd service.

Check logs with:

```sh
sudo journalctl -u ashes-wiki -f
```

Re-run the build and installer to deploy a new version. Existing environment configuration and database contents are preserved.

## 4. Choose network exposure

For LAN-only access, keep the app on `127.0.0.1` and use a reverse proxy, VPN, or SSH tunnel. Do not change it to `0.0.0.0` unless the network is trusted and firewalled.

### Pi-hole and Lighttpd on DietPi

When the DietPi host already provides Pi-hole DNS and Lighttpd, a split-DNS name can proxy to the local-only service without disturbing Lighttpd's default site:

```sh
pihole-FTL --config dns.hosts '[ "192.168.86.199 pi.hole", "192.168.86.199 aok.glyphforged.com" ]'
pihole reloaddns

lighty-enable-mod proxy
install -m 0644 deploy/lighttpd-aok.conf /etc/lighttpd/conf-available/99-ashes-wiki.conf
ln -sfn /etc/lighttpd/conf-available/99-ashes-wiki.conf /etc/lighttpd/conf-enabled/99-ashes-wiki.conf
lighttpd -tt -f /etc/lighttpd/lighttpd.conf
systemctl reload lighttpd
```

Keep `BIND_ADDRESS=127.0.0.1:3000` in `/etc/ashes-wiki/ashes-wiki.env`. Clients using the Pi-hole resolver can then open `http://aok.glyphforged.com`. This name is private DNS only; it does not make the wiki publicly accessible.

For a public domain, install Caddy and copy `deploy/Caddyfile.example` into your Caddy configuration. Replace the domain and generate a password hash with `caddy hash-password`. Caddy terminates HTTPS and protects the entire wiki with HTTP basic authentication before proxying to the local app.

The application itself still has no user accounts or authorization. A `GM SECRET` badge is not security, so never publish the app without proxy authentication or a private VPN such as Tailscale/WireGuard.

## 5. Backups and restore

Create a transactionally consistent online backup:

```sh
sudo ashes-wiki-backup
```

Backups default to `/var/backups/ashes-wiki`. Copy them off the board regularly; an SD card is not a backup destination.

To restore:

```sh
sudo systemctl stop ashes-wiki
sudo gunzip -c /path/to/ashes-TIMESTAMP.db.gz | sudo tee /var/lib/ashes-wiki/ashes.db >/dev/null
sudo chown ashes-wiki:ashes-wiki /var/lib/ashes-wiki/ashes.db
sudo chmod 0640 /var/lib/ashes-wiki/ashes.db
sudo systemctl start ashes-wiki
```

Verify `/healthz`, a search, and a known page after restoration.
