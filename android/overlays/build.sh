#!/usr/bin/env bash
# Builds the code-free APKs of the bundle into ~/.cache/crownos-android/agent/out/:
#   CrownFrameworkOverlay.apk  static RRO for android: no wallpaper service
#   CrownSystemUIOverlay.apk   static RRO for com.android.systemui: no freeform decor (safe-mode fallback)
#   CrownHome.apk              the blank HOME, org.crownos.android.home
#
# Usage: build.sh [keystore.p12]
#   With no argument the APKs are signed with a development key, created on first use under
#   ~/.cache/crownos-android/agent/keys/. A release keystore needs CROWN_KEYSTORE_PASS in the
#   environment; CROWN_KEY_ALIAS selects its key (default: crownos).
set -euo pipefail

overlays_dir="$(cd "$(dirname "$0")" && pwd)"
sdk_root="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
build_tools="$sdk_root/build-tools/37.0.0"
android_jar="$sdk_root/platforms/android-35/android.jar"
cache="$HOME/.cache/crownos-android/agent"
out="${CROWN_AGENT_OUT:-$cache/out}"
work="$cache/overlays-build"
dev_keystore="$cache/keys/dev.p12"
dev_password="crownos-dev"
key_alias="${CROWN_KEY_ALIAS:-crownos}"

keystore="${1:-$dev_keystore}"
if [[ "$keystore" == "$dev_keystore" ]]; then
    export CROWN_KEYSTORE_PASS="${CROWN_KEYSTORE_PASS:-$dev_password}"
    if [[ ! -f "$keystore" ]]; then
        mkdir -p "$(dirname "$keystore")"
        keytool -genkeypair -keystore "$keystore" -storetype PKCS12 -storepass:env CROWN_KEYSTORE_PASS \
            -alias "$key_alias" -keyalg RSA -keysize 3072 -validity 10000 -dname "CN=CrownOS development"
    fi
elif [[ ! -f "$keystore" ]]; then
    echo "error: keystore $keystore does not exist" >&2
    exit 1
elif [[ -z "${CROWN_KEYSTORE_PASS:-}" ]]; then
    echo "error: set CROWN_KEYSTORE_PASS for $keystore" >&2
    exit 1
fi

build_apk() {
    local source_name="$1" apk_name="$2"
    local source="$overlays_dir/$source_name" stage="$work/$source_name"
    mkdir -p "$stage"
    "$build_tools/aapt2" compile --dir "$source/res" -o "$stage/res.zip"
    "$build_tools/aapt2" link -I "$android_jar" --manifest "$source/AndroidManifest.xml" \
        --min-sdk-version 33 --target-sdk-version 33 -o "$stage/unaligned.apk" "$stage/res.zip"
    "$build_tools/zipalign" -f -p 4 "$stage/unaligned.apk" "$stage/aligned.apk"
    "$build_tools/apksigner" sign --v4-signing-enabled false --ks "$keystore" --ks-key-alias "$key_alias" \
        --ks-pass env:CROWN_KEYSTORE_PASS --out "$out/$apk_name" "$stage/aligned.apk"
    "$build_tools/apksigner" verify "$out/$apk_name"
    echo "built $out/$apk_name"
}

rm -rf "$work"
mkdir -p "$work" "$out"
build_apk framework-nowallpaper CrownFrameworkOverlay.apk
build_apk systemui-nodecor CrownSystemUIOverlay.apk
build_apk blank-home CrownHome.apk
