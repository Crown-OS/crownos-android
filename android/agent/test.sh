#!/usr/bin/env bash
# Compiles the Android-free agent packages with their tests for the host JVM and runs them.
# Compiling without android.jar proves these packages never touch the Android framework.
set -euo pipefail

agent_dir="$(cd "$(dirname "$0")" && pwd)"
vectors="$agent_dir/../../contracts/agent-wire-vectors.txt"
work="${CROWN_AGENT_WORK:-$HOME/.cache/crownos-android/agent}/host-test"
pure_packages=(wire atlas sizing policy keepalive parking)

rm -rf "$work"
mkdir -p "$work/classes"

sources=()
for package in "${pure_packages[@]}"; do
    mapfile -t -O "${#sources[@]}" sources < <(find "$agent_dir/src/org/crownos/agent/$package" -name '*.java' | sort)
done
mapfile -t -O "${#sources[@]}" sources < <(find "$agent_dir/test" -name '*.java' | sort)

javac --release 8 -encoding UTF-8 -Xlint:all -Xlint:-options -Werror -d "$work/classes" "${sources[@]}"
java -cp "$work/classes" org.crownos.agent.testing.TestRunner "$vectors"
