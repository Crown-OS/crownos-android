#!/usr/bin/env bash
# Prints a markdown table of the kernel options Waydroid needs, one column per config.
#
#   kernel-check.sh CONFIG...
#
# A CONFIG is a plain .config, a gzip one (/proc/config.gz) or a vmlinuz with IKCONFIG.

set -euo pipefail

options=(
    ANDROID_BINDER_IPC ANDROID_BINDER_IPC_RUST ANDROID_BINDERFS ANDROID_BINDER_DEVICES RUST
    SW_SYNC SYNC_FILE DMA_SHARED_BUFFER
    DMABUF_HEAPS DMABUF_HEAPS_SYSTEM DMABUF_HEAPS_CMA DMABUF_SYSFS_STATS UDMABUF
    PSI PSI_DEFAULT_DISABLED ASHMEM MEMFD_CREATE SHMEM PAGE_SIZE_4KB PAGE_SHIFT
    TRANSPARENT_HUGEPAGE ZRAM ZSWAP ZSMALLOC
    CGROUPS CGROUP_FREEZER MEMCG CGROUP_PIDS CGROUP_DEVICE CGROUP_BPF CPUSETS CGROUP_SCHED
    USER_NS NET_NS PID_NS IPC_NS UTS_NS SECCOMP_FILTER BPF_SYSCALL
    VETH BRIDGE NF_TABLES NF_TABLES_INET NF_TABLES_IPV4 NFT_NAT NFT_MASQ NF_NAT NF_NAT_MASQUERADE
    NFT_COMPAT IP_NF_IPTABLES IP_NF_FILTER IP_NF_NAT IP_NF_MANGLE IP_NF_TARGET_MASQUERADE
    NETFILTER_XT_NAT NETFILTER_XT_TARGET_MASQUERADE NETFILTER_XT_TARGET_CHECKSUM
    DRM DRM_VIRTIO_GPU DRM_VGEM DRM_SCHED DRM_KMS_HELPER DMA_FENCE_TRACE
    FUSE_FS OVERLAY_FS BLK_DEV_LOOP EXT4_FS TUN
)

read_config() {
    case $(file -b --mime-type -- "$1") in
        application/gzip) zcat -- "$1" ;;
        text/*) cat -- "$1" ;;
        *) "$(compgen -G '/usr/lib/modules/*/build/scripts/extract-ikconfig' | head -n1)" "$1" ;;
    esac
}

declare -A value
names=()
for config in "$@"; do
    name=${config##*/}
    names+=("$name")
    while IFS='=' read -r key val; do
        value[$name/$key]=$val
    done < <(read_config "$config" | sed -nE 's/^CONFIG_([A-Z0-9_]+)=(.*)$/\1=\2/p; s/^# CONFIG_([A-Z0-9_]+) is not set$/\1=n/p')
done

printf '| option |'; printf ' %s |' "${names[@]}"; printf '\n|---|'; printf -- '---|%.0s' "${names[@]}"; printf '\n'
for option in "${options[@]}"; do
    printf '| %s |' "$option"
    for name in "${names[@]}"; do printf ' %s |' "${value[$name/$option]:--}"; done
    printf '\n'
done
