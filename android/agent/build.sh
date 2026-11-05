#!/usr/bin/env bash
# Builds crown-agent.jar, a dex jar for app_process, and puts it next to its init service:
#   ~/.cache/crownos-android/agent/out/crown-agent.jar
#   ~/.cache/crownos-android/agent/out/crown-agent.rc
# Hidden framework classes the agent subclasses compile against stubs/, which never enter the dex,
# so the device's own classes load at runtime. CROWN_AGENT_OUT overrides the output directory and
# ANDROID_SDK_ROOT the SDK.
set -euo pipefail

agent_dir="$(cd "$(dirname "$0")" && pwd)"
sdk_root="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
build_tools="$sdk_root/build-tools/37.0.0"
android_jar="$sdk_root/platforms/android-35/android.jar"
lambda_stubs="$build_tools/core-lambda-stubs.jar"
cache="$HOME/.cache/crownos-android/agent"
out="${CROWN_AGENT_OUT:-$cache/out}"
work="$cache/build"

rm -rf "$work"
mkdir -p "$work/stub-classes" "$work/classes" "$out"

mapfile -t stub_sources < <(find "$agent_dir/stubs" -name '*.java' | sort)
mapfile -t agent_sources < <(find "$agent_dir/src" -name '*.java' | sort)

javac -source 8 -target 8 -encoding UTF-8 -Xlint:-options -bootclasspath "$android_jar" \
    -d "$work/stub-classes" "${stub_sources[@]}"
javac -source 8 -target 8 -encoding UTF-8 -Xlint:all -Xlint:-options -Werror \
    -bootclasspath "$android_jar:$lambda_stubs" -classpath "$work/stub-classes" \
    -d "$work/classes" "${agent_sources[@]}"

mapfile -t agent_classes < <(find "$work/classes" -name '*.class' | sort)
"$build_tools/d8" --release --min-api 33 --lib "$android_jar" --classpath "$work/stub-classes" \
    --output "$work/crown-agent.jar" "${agent_classes[@]}"

if "$build_tools/dexdump" "$work/crown-agent.jar" | grep "Class descriptor" | grep -q "'Landroid/"; then
    echo "error: framework stub classes leaked into the dex" >&2
    exit 1
fi

install -m 0644 "$work/crown-agent.jar" "$out/crown-agent.jar"
install -m 0644 "$agent_dir/../init/crown-agent.rc" "$out/crown-agent.rc"
echo "built $out/crown-agent.jar and $out/crown-agent.rc"
