# crownos-android packaging

`rootfs/` mirrors where the `crownos-android` package installs each file
(`crownos-pkgbuilds/crownos-android/PKGBUILD` copies it as is). The Venus tier's
files live in `venus/` and ship in `crownos-android-venus`. `check.sh` verifies
everything here without root.

| File | Purpose |
|---|---|
| `usr/lib/systemd/system/crownos-android-setup.service` | first-boot setup (`run`), gated by `/var/lib/crownos/android/pending`; the installer enables it |
| `usr/lib/systemd/system/waydroid-container.service.d/crownos-android.conf` | `prepare-container` before every container start |
| `usr/lib/systemd/user/crown-androidd.socket` | the daemon's socket; enabling it globally also enables the path unit |
| `usr/lib/systemd/user/crown-androidd.path` | starts the daemon at login while setup is pending, for the progress notification |
| `usr/lib/systemd/user/crown-androidd.service` | the daemon |
| `usr/lib/systemd/user/crownos-android-session.service` | `waydroid session start`, started and stopped by the daemon |
| `usr/share/polkit-1/rules.d/50-crownos-android.rules` | active local sessions may start and stop `waydroid-container.service` and `crownos-android-setup.service` |
| `usr/share/polkit-1/actions/org.crownos.android.policy` | `org.crownos.android.setup`: `pkexec crownos-android-setup remove [--wipe-data]` |
| `usr/lib/tmpfiles.d/crownos-android.conf` | `/var/lib/crownos/android` |
| `usr/share/waydroid-extra/channels.cfg` | Waydroid's default channels point at an unresolvable host, so the unauthenticated D-Bus `Init` path can never install an image; setup passes its pinned channel with `-c`/`-v` |
| `usr/lib/NetworkManager/conf.d/50-crownos-android.conf` | NetworkManager leaves `waydroid0` alone |
| `usr/lib/firewalld/zones/crownos-android.xml` | firewalld trusts `waydroid0` |

## Android networking and host firewalls

Waydroid's `waydroid-net.sh` creates the `waydroid0` bridge (192.168.240.0/24),
runs dnsmasq on it for DHCP and DNS, and, because the waydroid package depends
on nftables, adds its own nftables tables: `inet lxc` accepts DHCP and DNS from
the bridge and forwarding through it, and `ip lxc` masquerades the bridge
network. An accept in one nftables table does not stop another table's base
chain on the same hook from dropping the packet, so any other firewall with a
dropping forward or input chain cuts Android off (Waydroid issue #2458):

- **firewalld:** the `crownos-android` zone above binds `waydroid0` with target
  ACCEPT, the same as `trusted`. Nothing to do.
- **Docker** sets the `ip filter FORWARD` policy to DROP. Accept the bridge in
  the chain Docker reserves for this:
  `iptables -I DOCKER-USER -i waydroid0 -j ACCEPT` and
  `iptables -I DOCKER-USER -o waydroid0 -j ACCEPT`, or set
  `"ip-forward-no-drop": true` in `/etc/docker/daemon.json` (Docker 28 and later).
- **ufw** drops forwarded traffic by default and filters input: `ufw allow in on
  waydroid0`, `ufw route allow in on waydroid0` and `ufw route allow out on waydroid0`.

`crownos-android-setup` reports which of these it finds while probing.
