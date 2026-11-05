# D1: kernel after the linux-cachyos 7.3 series

Decided 2026-11-05 by the user.

## Problem
- **The C binder goes away.** Upstream Linux removes the C binder in 7.4. The Rust binder cannot be built into linux-cachyos, because CachyOS builds with LTO and BTF.
- **The result:** a linux-cachyos 7.4 built the usual way has no binder, and Android support stops working.

Evidence is in `s3-platform.md`.

## Decision
- **Kernel:** ship `linux-cachyos-lts` (6.18, C binder, supported until December 2028) once linux-cachyos moves past 7.3.
- **NVIDIA:** keep `linux-cachyos-lts-nvidia-open` for NVIDIA machines.

## Consequences
- **Installer:** the installer and the ISO profile switch `base::KERNEL` and the mkinitcpio preset to `linux-cachyos-lts` before the first ISO built on a 7.4 linux-cachyos. That change belongs to lane L9.
- **Hardware support:** newer hardware support lags behind mainline. Revisit when the Rust binder passes the large-transaction test (#2422) in a kernel CrownOS can build.
- **Binder nodes:** the setup service pins the binder nodes to binder, vndbinder and hwbinder on every kernel (the `s3-platform` root step `binder-nodes`). Switching kernels then never leaves Android waiting for servicemanager.
