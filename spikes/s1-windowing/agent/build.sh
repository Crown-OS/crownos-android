#!/usr/bin/env bash
# Builds build/crown-agent-spike.jar (dex) from src/ against android-35 android.jar.
# stubs/ holds compile-only copies of hidden framework classes the agent subclasses;
# they are left out of the dex so the device's own framework classes are used at runtime.
set -euo pipefail

cd "$(dirname "$0")"

sdk_root="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
build_tools="$sdk_root/build-tools/37.0.0"
android_jar="$sdk_root/platforms/android-35/android.jar"
lambda_stubs="$build_tools/core-lambda-stubs.jar"
out=build

rm -rf "$out/stub-classes" "$out/classes" "$out/crown-agent-spike.jar"
mkdir -p "$out/stub-classes" "$out/classes"

mapfile -t stub_sources < <(find stubs -name '*.java' | sort)
mapfile -t agent_sources < <(find src -name '*.java' | sort)

javac -source 8 -target 8 -Xlint:-options -bootclasspath "$android_jar" \
    -d "$out/stub-classes" "${stub_sources[@]}"

javac -source 8 -target 8 -Xlint:all -Xlint:-options -Xlint:-deprecation -Werror \
    -bootclasspath "$android_jar:$lambda_stubs" -classpath "$out/stub-classes" \
    -d "$out/classes" "${agent_sources[@]}"

mapfile -t agent_classes < <(find "$out/classes" -name '*.class' | sort)
"$build_tools/d8" --release --min-api 33 --lib "$android_jar" --classpath "$out/stub-classes" \
    --output "$out/crown-agent-spike.jar" "${agent_classes[@]}"

if "$build_tools/dexdump" "$out/crown-agent-spike.jar" | grep "Class descriptor" | grep -q "'Landroid/"; then
    echo "error: framework stub classes leaked into the dex" >&2
    exit 1
fi

echo "built $(realpath "$out/crown-agent-spike.jar")"
