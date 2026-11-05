#!/usr/bin/env bash
# Creates a lane directory: a mirror of /e/CrownOS in which every entry is a
# symlink to the real checkout, except the repos the lane edits, which are git
# worktrees on the lane's branch. Relative path dependencies such as
# `../crownos-ipc` and `../vendor` keep resolving, and the user's own
# checkouts are never written to.
#
#   tools/lane.sh <lane> <base-ref-spec>...
#
# Each spec is <repo>[@<start>], where <start> is the branch or commit the
# lane branch starts from (default: the repo's current HEAD commit). Example:
#
#   tools/lane.sh compositor crownpositor crownos-protocols@android/base
#
# The lane branch is android/<lane>. Re-running only adds missing pieces.
set -euo pipefail

source_root=${CROWNOS_ROOT:-/e/CrownOS}
lanes_root=${CROWNOS_LANES:-$HOME/crownos-lanes}
lane=${1:?lane name}
shift
lane_dir=$lanes_root/$lane
mkdir -p "$lane_dir"

declare -A worktree_start=()
for spec in "$@"; do
    repo=${spec%@*}
    start=HEAD
    [[ $spec == *@* ]] && start=${spec#*@}
    worktree_start[$repo]=$start
done

# Siblings the lane only reads. A repo with an android/base worktree in
# $lanes_root/_base resolves to it, so every lane builds against the frozen
# contracts rather than whatever the user's checkout holds.
for entry in "$source_root"/* "$source_root"/.[!.]*; do
    [[ -e $entry ]] || continue
    name=$(basename "$entry")
    [[ -n ${worktree_start[$name]+set} ]] && continue
    [[ -d $lanes_root/_base/$name && ! -L $lanes_root/_base/$name ]] && entry=$lanes_root/_base/$name
    [[ -e $lane_dir/$name || -L $lane_dir/$name ]] || ln -s "$entry" "$lane_dir/$name"
done

for repo in "${!worktree_start[@]}"; do
    target=$lane_dir/$repo
    [[ -L $target ]] && rm "$target"
    [[ -d $target ]] && continue
    branch=android/$lane
    if git -C "$source_root/$repo" show-ref --verify --quiet "refs/heads/$branch"; then
        git -C "$source_root/$repo" worktree add -q "$target" "$branch"
    else
        git -C "$source_root/$repo" worktree add -q -b "$branch" "$target" "${worktree_start[$repo]}"
    fi
    echo "$repo: worktree on $branch at $target"
done
echo "lane $lane ready at $lane_dir"
