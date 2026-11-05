#!/usr/bin/env python3
"""Binder facts on the running kernel: driver, binderfs, the nodes Waydroid uses, and kernel errors.

Works as the user; as root (`root-steps.sh binder-check`) it also reads the kernel log through dmesg.
"""

import configparser
import os
import platform
import re
from pathlib import Path

import common

NODE_NAMES = ("binder", "vndbinder", "hwbinder", "anbox-binder", "anbox-vndbinder", "anbox-hwbinder")
WAYDROID_CFG = "/var/lib/waydroid/waydroid.cfg"
CONFIG_NODES = "/var/lib/waydroid/lxc/waydroid/config_nodes"
BINDER_FAILURE = re.compile(r"binder.*(fail|ENOSPC|ENOMEM|error)", re.IGNORECASE)


def node_state(name: str) -> dict:
    path = Path("/dev") / name
    return {"symlink": os.readlink(path) if path.is_symlink() else None,
            "exists": path.exists(), "dangling": path.is_symlink() and not path.exists()}


def binder_driver() -> str | None:
    drivers = {"rust_binder": "rust", "binder": "c", "binder_linux": "out-of-tree binder_linux"}
    return next((kind for module, kind in drivers.items() if Path("/sys/module", module).exists()), None)


def waydroid_names() -> dict | None:
    config = configparser.ConfigParser()
    if not config.read(WAYDROID_CFG):
        return None
    section = config["waydroid"] if config.has_section("waydroid") else {}
    return {key: section.get(key) for key in ("binder", "vndbinder", "hwbinder", "binder_protocol")}


def kernel_failures() -> dict:
    dmesg = common.run(["dmesg", "--notime"])
    if dmesg.returncode != 0:
        return {"error": dmesg.stderr.strip()[:200]}
    lines = [line for line in dmesg.stdout.splitlines() if BINDER_FAILURE.search(line)]
    return {"count": len(lines), "last": lines[-5:], "enospc": sum("ENOSPC" in line for line in lines)}


def main() -> None:
    filesystems = common.read_text("/proc/filesystems") or ""
    mounts = [line.split()[4] for line in (common.read_text("/proc/self/mountinfo") or "").splitlines()
              if " - binder " in line]
    binderfs = Path("/dev/binderfs")
    common.emit("binder_check", {
        "kernel": platform.release(),
        "binder_driver": binder_driver(),
        "binderfs_registered": any(line.split()[-1] == "binder" for line in filesystems.splitlines() if line),
        "binderfs_mounts": mounts,
        "binderfs_entries": sorted(entry.name for entry in binderfs.iterdir()) if binderfs.is_dir() else None,
        "dev_nodes": {name: node_state(name) for name in NODE_NAMES},
        "waydroid_cfg": waydroid_names(),
        "config_nodes_binder": [line.strip() for line in (common.read_text(CONFIG_NODES) or "").splitlines()
                                if "binder" in line],
        "kernel_failures": kernel_failures() if os.geteuid() == 0 else "needs root",
    })


if __name__ == "__main__":
    main()
