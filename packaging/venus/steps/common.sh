# Shared helpers of build.sh. Sourced, never run.

readonly HOST_TOOLS=(curl git gzip meson ninja patch pkg-config glslangValidator python3 sha256sum tar zstd)

die() {
    echo "build.sh: $*" >&2
    exit 1
}

step() {
    echo "== $*"
}

sha256_of() {
    sha256sum "$1" | cut -d' ' -f1
}

check_host_tools() {
    local tool
    for tool in "${HOST_TOOLS[@]}"; do
        command -v "$tool" >/dev/null || die "missing host tool: $tool"
    done
    python3 -c 'import mako, yaml' 2>/dev/null \
        || die "python3 needs mako and yaml (pacman -S python-mako python-yaml, or activate a venv that has them)"
}

check_ndk() {
    local properties=$ANDROID_NDK/source.properties
    [[ -f $properties ]] || die "no NDK at $ANDROID_NDK (set ANDROID_NDK)"
    grep -qx "Pkg.Revision = $NDK_REVISION" "$properties" \
        || die "$ANDROID_NDK is not NDK $NDK_REVISION: $(grep Pkg.Revision "$properties")"
}
