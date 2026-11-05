# Host build: the patched virglrenderer with its vtest server and venus render
# server, laid out for /usr/lib/crownos-android/venus. Sourced by build.sh.

readonly HOST_PREFIX=/usr
readonly HOST_DIR=lib/crownos-android/venus
readonly HOST_FILES=(virgl_test_server virgl_render_server libvirglrenderer.so.1)

# build_render_server <patched virglrenderer tree>. The compiled-in render
# server path is $HOST_PREFIX/$HOST_DIR/virgl_render_server, and the
# $ORIGIN rpath keeps virgl_test_server off Arch's own libvirglrenderer.
build_render_server() {
    local virgl=$1 build=$WORK/build/virglrenderer stage=$WORK/stage/virglrenderer file
    rm -rf "$build" "$stage"
    mkdir -p "$build"
    meson setup "$build" "$virgl" \
        --prefix="$HOST_PREFIX" --bindir="$HOST_DIR" --libdir="$HOST_DIR" --libexecdir="$HOST_DIR" \
        -Dbuildtype=release -Dvenus=true -Drender-server-worker=process \
        -Dplatforms=egl -Dvideo=false -Dtests=false \
        -Dc_link_args="-Wl,-rpath,\$ORIGIN" >"$build/setup.log" \
        || die "meson setup failed for virglrenderer, see $build/setup.log"
    ninja -C "$build" -j "$JOBS"
    meson install -C "$build" --destdir "$stage" --strip --quiet
    for file in "${HOST_FILES[@]}"; do
        install -D -m 0755 "$stage$HOST_PREFIX/$HOST_DIR/$file" "$OUT/host/$file"
    done
}
