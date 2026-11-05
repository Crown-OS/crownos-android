# The output tree check, its checksums and the release tarball the PKGBUILD
# pins. Sourced by build.sh.

# The overlay half matches venus::overlay_files().
readonly OUTPUT_FILES=(
    overlay/vendor/lib64/hw/vulkan.virtio.so
    overlay/vendor/lib/hw/vulkan.virtio.so
    overlay/vendor/lib64/libgbm_mesa_wrapper.so
    overlay/vendor/lib/libgbm_mesa_wrapper.so
    host/virgl_test_server
    host/virgl_render_server
    host/libvirglrenderer.so.1
)

# install_licenses <mesa tree> <virglrenderer tree> <minigbm tree> <patch dir>:
# the licences of the code compiled into the output, under licenses/.
install_licenses() {
    install -D -m 0644 "$1/docs/license.rst" "$OUT/licenses/mesa-license.rst"
    install -D -m 0644 "$2/COPYING" "$OUT/licenses/virglrenderer-COPYING"
    install -D -m 0644 "$3/LICENSE" "$OUT/licenses/minigbm-LICENSE"
    install -D -m 0644 "$4/waydroid-nvidia-LICENSE" "$OUT/licenses/waydroid-nvidia-LICENSE"
}

check_output_tree() {
    local expected actual
    expected=$(printf '%s\n' "${OUTPUT_FILES[@]}" | LC_ALL=C sort)
    actual=$(cd "$OUT" && find overlay host -type f | LC_ALL=C sort)
    [[ $expected == "$actual" ]] || die "unexpected output tree in $OUT:
$(diff <(echo "$expected") <(echo "$actual"))"
}

write_checksums() {
    (cd "$OUT" && sha256sum "${OUTPUT_FILES[@]}" >SHA256SUMS)
}

# pack_release: a byte-reproducible $WORK/dist/<name>.tar.zst of the output
# tree under <name>/, printed with its sha256.
pack_release() {
    local name=crownos-android-venus-$VENUS_RELEASE
    local tarball=$WORK/dist/$name.tar.zst
    mkdir -p "$WORK/dist"
    tar -C "$OUT" --sort=name --format=gnu --mtime="@$SOURCE_DATE_EPOCH" \
        --owner=0 --group=0 --numeric-owner --mode=u=rwX,go=rX \
        --transform="s|^|$name/|" -cf - SHA256SUMS licenses overlay host \
        | zstd -q -19 -T1 -f -o "$tarball"
    echo "$(sha256_of "$tarball")  $tarball"
}
