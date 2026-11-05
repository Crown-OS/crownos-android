#!/usr/bin/env bash
# Builds the code-free spike APKs into build/:
#   CrownSpikeSystemUIOverlay.apk   mutable RRO for com.android.systemui (WM Shell decor dimens = 0)
#   CrownSpikeFrameworkOverlay.apk  static RRO for android (config_enableWallpaperService = false)
#   CrownSpikeHome.apk              HOME activity backed by android.app.Activity, floating and empty
# APKs are signed with a throwaway key kept outside the repo.
set -euo pipefail

cd "$(dirname "$0")"

sdk_root="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
build_tools="$sdk_root/build-tools/37.0.0"
android_jar="$sdk_root/platforms/android-35/android.jar"
key_dir="${CROWN_SPIKE_KEY_DIR:-$HOME/.cache/crownos-android/s1/keys}"
keystore="$key_dir/spike.p12"
keystore_password="crownos-spike"
out=build

if [[ ! -f "$keystore" ]]; then
    mkdir -p "$key_dir"
    keytool -genkeypair -keystore "$keystore" -storetype PKCS12 -storepass "$keystore_password" \
        -alias spike -keyalg RSA -keysize 2048 -validity 3650 -dname "CN=CrownOS S1 spike"
fi

build_apk() {
    local source_dir="$1" apk_name="$2"
    local work="$out/$source_dir"
    rm -rf "$work"
    mkdir -p "$work"
    "$build_tools/aapt2" compile --dir "$source_dir/res" -o "$work/res.zip"
    "$build_tools/aapt2" link -I "$android_jar" --manifest "$source_dir/AndroidManifest.xml" \
        --min-sdk-version 33 --target-sdk-version 33 -o "$work/unaligned.apk" "$work/res.zip"
    "$build_tools/zipalign" -f -p 4 "$work/unaligned.apk" "$work/aligned.apk"
    "$build_tools/apksigner" sign --v4-signing-enabled false --ks "$keystore" --ks-pass "pass:$keystore_password" \
        --out "$out/$apk_name" "$work/aligned.apk"
    echo "built $(realpath "$out/$apk_name")"
}

mkdir -p "$out"
build_apk systemui-nodecor CrownSpikeSystemUIOverlay.apk
build_apk framework-nowallpaper CrownSpikeFrameworkOverlay.apk
build_apk blank-home CrownSpikeHome.apk
