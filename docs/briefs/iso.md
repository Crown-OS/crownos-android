# Lane `iso`: packaging, units and ISO integration

## Repos and owned paths
- **crownos-android**, worktree `~/crownos-lanes/iso/crownos-android`, branch `android/iso`. You own `packaging/`, except `packaging/venus/`, which the venus lane owns.
- **crownos-pkgbuilds**, worktree `~/crownos-lanes/iso/crownos-pkgbuilds`, branch `android/iso`. You own `crownos-android/`.
- **crownos-iso**, worktree `~/crownos-lanes/iso/crownos-iso`, branch `android/iso`. The profile, config, scripts and tests are yours to edit for this feature.

## Goal
Installing CrownOS with the Android switch on yields a system where everything starts correctly with no manual step.

## Requirements

### 1. Units and files, in `packaging/`
Names and paths come from contract 10.
- **`crownos-android-setup.service`:**
  - `Type=oneshot`, `ConditionPathExists=/var/lib/crownos/android/pending`;
  - `After=network-online.target`, `Wants=network-online.target`;
  - `Nice=19`, `IOSchedulingClass=idle`, `CPUWeight=20`;
  - `ExecStart=/usr/lib/crownos-android/crownos-android-setup run`;
  - `WantedBy=multi-user.target`;
  - hardening that still allows its work: ProtectHome is off for `remove --wipe-data` only, so keep `run` hardened and use a separate service if needed.
- **`waydroid-container.service.d/crownos-android.conf`:** `ExecStartPre=/usr/lib/crownos-android/crownos-android-setup prepare-container`.
- **`crown-androidd.socket`:** `ListenStream=%t/crownos/crownos-android.sock`, `WantedBy=sockets.target`. Plus **`crown-androidd.service`**, which must not stop the session unit when it exits.
- **`crownos-android-session.service`:** user unit, `ExecStart=/usr/bin/waydroid session start`, `ExecStop=/usr/bin/waydroid session stop`, environment from the user manager, no `[Install]` (the daemon starts it).
- **Polkit:**
  - A JS rule granting active local sessions `org.freedesktop.systemd1.manage-units` for `waydroid-container.service` and `crownos-android-setup.service` only (start and stop).
  - An action `org.crownos.android.setup` for `pkexec /usr/lib/crownos-android/crownos-android-setup`, `auth_admin_keep`, with exec.path annotation (pattern: `crownos-iso/packaging/org.crownos.appstore.policy`).
- **tmpfiles:** `/var/lib/crownos/android` 0755 root.
- **`channels.cfg`:** for `/usr/share/waydroid-extra/channels.cfg`, the CrownOS channel URLs. These are the file:// channel the setup service writes, so `waydroid init` needs no polkit prompt; read `/usr/lib/waydroid/tools/actions/initializer.py` for the exact keys and semantics.
- **Network:** `waydroid0` is unmanaged in NetworkManager and trusted in firewalld (drop-in files), and there is an nftables-compatible NAT note (`s3-platform.md`, and Waydroid issue #2458 for docker/ufw).
- **Memory tuning** (S3): `/usr/lib/sysctl.d/90-crownos-memory.conf` with `vm.swappiness=100` and `vm.page-cluster=0`, plus the MGLRU `min_ttl_ms` setting through a tmpfiles write to `/sys/kernel/mm/lru_gen/min_ttl_ms`. Put these in the crownos-desktop meta package (`crownos-iso/packaging/`), not the Android package.

### 2. `crownos-pkgbuilds/crownos-android/PKGBUILD` and `.install`
- **Build:** the crownos-android workspace bins (`crown-androidd`, `crown-android`, `crownos-android-setup`), following `crownos-polkit-agent/PKGBUILD` (cargo fetch --locked, frozen build).
- **Install:**
  - the bins to `/usr/bin` and `/usr/lib/crownos-android/`;
  - the units, polkit files, tmpfiles and channels.cfg;
  - the bundle tree from a `bundle-<version>.tar.zst` source artifact into `/usr/share/crownos-android/bundle/<version>/`.
- **Depends:** `waydroid lxc python-gbinder libgbinder libglibutil dnsmasq crownotify`.
- **`.install`:** `systemctl --global enable crown-androidd.socket` and `systemctl daemon-reload` hints, with the voice package as the pattern.

### 3. crownos-iso
- **`profile/install.packages`:** add `waydroid lxc python-gbinder libgbinder libglibutil dnsmasq crownos-android crownos-android-venus`. `scripts/mirror.sh` resolves their dependencies into the offline repo.
- **`components.txt`:** add the crownos-android component the way the others are listed.
- **Compositor startup:** add `crownotify` to `config/crownos/compositor.ron` `startup`. Today nothing starts crownotify on installed systems.
- **Tests:** add `tests/autoinstall-android.json`, the autoinstall config enabling Android with Google Play. Extend `scripts/install-test.sh` with an optional Android pass that checks, inside the installed image after install:
  - `crownos-android-setup.service` is enabled;
  - the global user socket is enabled;
  - `/var/lib/crownos/android/pending` and `/etc/crownos/android.ron` exist;
  - after boot, `setup.json` reports `waiting_for_network` (the test VM has no network).

  Write it; do not run it.
- **Kernel:** follow `d1-kernel.md`. Add a short `docs/kernel.md` (or a README section) saying the switch to `linux-cachyos-lts` happens before the first ISO on a 7.4 linux-cachyos. Change nothing in the kernel today.

## Acceptance
- **PKGBUILD:** `namcap`-clean if namcap is installed, and `makepkg --printsrcinfo` works.
- **Units:** `systemd-analyze verify` passes on the unit files where possible without root.
- **Polkit:** the rules parse (`pkaction` is not available unprivileged, so validate the JS syntax with node or rhino if present, or keep it trivially simple).
- **Scripts:** `bash -n` and shellcheck clean on the scripts.
- **Commits:** on `android/iso` in all three worktrees.
