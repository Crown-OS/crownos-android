# The CrownOS Android overlay bundle

The bundle is everything CrownOS adds inside Android to make Android windows
native: the hwcomposer fork, the `crown-agent` and its init service, the
resource overlays and the blank HOME. It is installed into
`/var/lib/waydroid/overlay` by `crownos-android-setup run`, never into an image.

## Layout

```
<version>/
  manifest.json            contract 8: files, sha256, modes, whiteouts, props, image pins
  manifest.json.minisig    minisign signature of manifest.json
  whiteouts.txt            optional: overlay paths to hide, one per line
  vendor/lib64/hw/hwcomposer.waydroid.so
  system/framework/crown-agent.jar
  system/etc/init/crown-agent.rc
  system/product/overlay/*.apk
  system/product/app/CrownHome/CrownHome.apk
```

The `crownos-android` package ships the tree under
`/usr/share/crownos-android/bundle/<version>/`. Setup trusts nothing in it until
the manifest verifies against the key in `setup/src/keys.rs`, then checks every
file's sha256 while copying it.

## Building

1. Build the parts, through the build slot:
   - `crownos-waydroid-hwc`: `ndk/build.sh`, which leaves
     `~/.cache/crownos-android/hwc/out/x86_64/vendor/lib64/hw/hwcomposer.waydroid.so`;
   - `android/agent/build.sh` and `android/overlays/build.sh`, which leave
     `crown-agent.jar`, `crown-agent.rc`, `CrownHome.apk` and the overlay APKs in
     `~/.cache/crownos-android/agent/out/`.
2. Assemble, write the manifest and sign:

   ```sh
   ~/crownos-lanes/bin/build-slot overlay/build-bundle.sh --version 2026.11.0 \
       --key ~/.cache/crownos-android/keys/crownos-android-dev.key
   ```

   The result is `~/.cache/crownos-android/bundle/2026.11.0/` and
   `bundle-2026.11.0.tar.zst`, the source artifact of the `crownos-android`
   PKGBUILD. The archive holds the tree's contents, with `manifest.json` and
   `manifest.json.minisig` at its root, so the PKGBUILD unpacks it straight
   into `bundle/<version>/`. Without `--key` the tree is built unsigned, for
   inspection only.
   `HWC_OUT`, `AGENT_OUT`, `--out`, `--whiteouts` and `--setup <binary>`
   override the defaults. A missing artifact stops the build with the command
   that produces it.

`build-bundle.sh` calls `crownos-android-setup make-manifest`, which hashes
every file under `system/` and `vendor/`, records its mode, reads
`whiteouts.txt`, adds the contract 11 properties and copies the image pins
from `pins.json`.

## Image pins

`pins.json` names the `waydroid` package version and, per flavor, the exact
system and vendor builds the bundle was tested with. Setup downloads only
these, and Waydroid's updater only ever sees them, through the file channel
setup writes. `update-pins.sh` regenerates the file from Waydroid's OTA JSON:

```sh
overlay/update-pins.sh --system-datetime 1775208984 --vendor-datetime 1777409843
```

It takes the GAPPS system build with that build time and the VANILLA build of
the same day.

### Moving to a new image pin

The HWC links against libraries extracted from the pinned images, so a new pin
means a new HWC build:

1. Run `update-pins.sh` with the new build times and review the diff.
2. Extract the new `system.img` and `vendor.img` as `crownos-waydroid-hwc`'s
   `ndk/README.md` describes, then re-run its `ndk/build.sh` against them.
   Every undefined symbol must resolve.
3. Rebuild the agent if the framework changed, then run `build-bundle.sh` with
   a new `--version`. The version is reported in the HWC and agent handshake,
   so a mismatch puts Android into safe mode instead of running a mixed set.
4. Test on hardware before the package ships the new bundle.

## Signing keys

Bundles are signed with [minisign](https://jedisct1.github.io/minisign/).
Setup verifies them with `minisign-verify` against `PUBLIC_KEY` in
`setup/src/keys.rs`, accepting only prehashed (`ED`) signatures, which is what
`minisign -S` makes.

### The development key

`keys.rs` holds a placeholder development key, `crownos-android-dev`, key id
`013CAECD11A053F5`. It is an unencrypted minisign key pair, in the format
`minisign -G -W` writes. Its files are outside the repository:

- `~/.cache/crownos-android/keys/crownos-android-dev.key` (secret, mode 0600)
- `~/.cache/crownos-android/keys/crownos-android-dev.pub`

It exists so test machines can run signed development bundles. It must never
sign a release. To replace it:

```sh
minisign -G -W -p ~/.cache/crownos-android/keys/crownos-android-dev.pub \
    -s ~/.cache/crownos-android/keys/crownos-android-dev.key
printf 'crownos-android dev key' > /tmp/devkey-check
minisign -S -s ~/.cache/crownos-android/keys/crownos-android-dev.key \
    -m /tmp/devkey-check -t 'crownos-android dev key check'
```

Then put the second line of the `.pub` file into `PUBLIC_KEY`, and the
contents of `/tmp/devkey-check.minisig` into the test in `keys.rs`, which
proves the committed key has a secret that someone holds.

### The release key

- **Generate** it on an offline machine with a strong passphrase, so the
  secret is scrypt-encrypted: `minisign -G -p crownos-android.pub -s crownos-android.key`.
- **Store** the secret key only offline, on two encrypted media kept apart.
  It never goes into a repository, a CI secret store or a build machine that
  is online.
- **Ship** the public key by replacing `PUBLIC_KEY` and the `keys.rs` test
  fixture with the release key's, in the same release that first carries a
  release-signed bundle. A release build that still holds the development key
  is not a release.
- **Sign** on the offline machine: copy the unsigned tree there, run
  `minisign -S -s crownos-android.key -m manifest.json -t "crownos-android bundle <version>"`,
  check it with `minisign -V -p crownos-android.pub -m manifest.json`, and copy
  `manifest.json.minisig` back. Then pack the tarball from inside the tree,
  as `build-bundle.sh` does (`tar -C <tree> ... .`).
- **Rotate** by shipping a `crownos-android` package whose `keys.rs` holds the
  new key, together with a bundle signed by it. The binary and the bundle
  travel in one package, so they never disagree.
- **If the key leaks**, rotate at once. Bundles signed by the old key stop
  verifying as soon as the new package is installed.
