# Pinned source tarballs and patched source trees. Sourced by build.sh.

# fetch <file name> <url> <sha256>: prints the path of the verified tarball in
# $DOWNLOADS, downloading it when it is missing or does not match.
fetch() {
    local file=$DOWNLOADS/$1 url=$2 sha256=$3
    if [[ ! -f $file || $(sha256_of "$file") != "$sha256" ]]; then
        curl -fsSL --retry 3 --proto '=https' -o "$file.part" "$url" || die "download failed: $url"
        mv "$file.part" "$file"
    fi
    [[ $(sha256_of "$file") == "$sha256" ]] || die "sha256 mismatch: $file (expected $sha256)"
    echo "$file"
}

# unpack_patched <tree> <git archive> <commit> <patch dir>: a fresh tree of
# the pinned commit with every patch of <patch dir> applied in order.
unpack_patched() {
    local tree=$1 archive=$2 commit=$3 patches=$4 patch_file
    [[ $(gzip -dc "$archive" | git get-tar-commit-id) == "$commit" ]] \
        || die "$archive is not an archive of commit $commit"
    rm -rf "$tree"
    mkdir -p "$tree"
    tar -xzf "$archive" -C "$tree" --strip-components=1
    for patch_file in "$patches"/*.patch; do
        patch -d "$tree" -Np1 --fuzz=0 --quiet -i "$patch_file" \
            || die "$(basename "$patch_file") does not apply to $commit"
    done
}

# source_tree <project> <commit>: where the patched tree of a project lives.
# The relative path from a build dir to it ends up in __FILE__ strings, so it
# must stay stable for byte-identical rebuilds.
source_tree() {
    echo "$WORK/src/$1-${2:0:8}-src"
}
