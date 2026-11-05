#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use venus::{MIN_DRIVER_MAJOR, Support, detect};

const OPEN_MODULE_615: &str = "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  615.78.08  Release Build  (root@)  \nGCC version:  gcc version 16.2.1 20260810 (GCC) \n";
const CLOSED_MODULE_615: &str = "NVRM version: NVIDIA UNIX x86_64 Kernel Module  615.78.08  Release Build  (builder@host)  Tue Oct  6 12:00:00 UTC 2026\nGCC version:  gcc version 16.2.1 20260810 (GCC) \n";
const BOOT_VGA: &str = "1";
const NOT_BOOT_VGA: &str = "0";

/// A sysfs and procfs tree under the test's own temp directory.
struct FakeMachine {
    root: PathBuf,
}

impl FakeMachine {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("venus-detect")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    /// The NVIDIA-only machine Venus is for: one open-module 615 GPU with
    /// modeset and udmabuf.
    fn nvidia_only(name: &str) -> Self {
        let machine = Self::new(name);
        machine.gpu("card0", "nvidia", Some(BOOT_VGA), "renderD128");
        machine.write("proc/driver/nvidia/version", OPEN_MODULE_615);
        machine.write("sys/module/nvidia_drm/parameters/modeset", "Y\n");
        machine.write("sys/class/misc/udmabuf/dev", "10:258\n");
        machine
    }

    fn gpu(&self, card: &str, driver: &str, boot_vga: Option<&str>, render_node: &str) {
        let device = format!("sys/class/drm/{card}/device");
        self.write(
            &format!("{device}/uevent"),
            &format!("DRIVER={driver}\nPCI_CLASS=30000\n"),
        );
        if let Some(boot_vga) = boot_vga {
            self.write(&format!("{device}/boot_vga"), &format!("{boot_vga}\n"));
        }
        self.write(&format!("{device}/drm/{card}/dev"), "226:0\n");
        self.write(&format!("{device}/drm/{render_node}/dev"), "226:128\n");
        self.write(&format!("sys/class/drm/{card}-eDP-1/status"), "connected\n");
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn remove(&self, relative: &str) {
        let path = self.root.join(relative);
        if path.is_dir() {
            fs::remove_dir_all(path).unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
    }

    fn detect(&self) -> Option<Support> {
        detect(&self.root.join("sys"), &self.root.join("proc")).unwrap()
    }
}

fn supported_on(render_node: &str) -> Option<Support> {
    Some(Support {
        render_node: PathBuf::from(render_node),
        driver_version: "615.78.08".into(),
    })
}

#[test]
fn an_nvidia_only_machine_runs_venus_on_its_render_node() {
    let machine = FakeMachine::nvidia_only("nvidia-only");
    assert_eq!(machine.detect(), supported_on("/dev/dri/renderD128"));
}

#[test]
fn a_hybrid_laptop_with_the_amd_igpu_as_boot_vga_has_no_venus() {
    let machine = FakeMachine::nvidia_only("hybrid-igpu");
    machine.remove("sys/class/drm/card0");
    machine.gpu("card0", "nvidia", Some(NOT_BOOT_VGA), "renderD129");
    machine.gpu("card1", "amdgpu", Some(BOOT_VGA), "renderD128");
    assert_eq!(machine.detect(), None);
}

#[test]
fn a_hybrid_laptop_in_dgpu_mux_mode_runs_venus_on_the_nvidia_node() {
    let machine = FakeMachine::nvidia_only("hybrid-dgpu-mux");
    machine.remove("sys/class/drm/card0");
    machine.gpu("card0", "nvidia", Some(BOOT_VGA), "renderD129");
    machine.gpu("card1", "amdgpu", Some(NOT_BOOT_VGA), "renderD128");
    assert_eq!(machine.detect(), supported_on("/dev/dri/renderD129"));
}

#[test]
fn an_amd_only_machine_has_no_venus() {
    let machine = FakeMachine::nvidia_only("amd-only");
    machine.remove("sys/class/drm/card0");
    machine.remove("proc/driver/nvidia");
    machine.gpu("card0", "amdgpu", Some(BOOT_VGA), "renderD128");
    assert_eq!(machine.detect(), None);
}

#[test]
fn the_closed_kernel_module_has_no_venus() {
    let machine = FakeMachine::nvidia_only("closed-module");
    machine.write("proc/driver/nvidia/version", CLOSED_MODULE_615);
    assert_eq!(machine.detect(), None);
}

#[test]
fn a_driver_older_than_the_minimum_has_no_venus() {
    let machine = FakeMachine::nvidia_only("old-driver");
    let old = MIN_DRIVER_MAJOR - 5;
    machine.write(
        "proc/driver/nvidia/version",
        &OPEN_MODULE_615.replace("615.78.08", &format!("{old}.82.09")),
    );
    assert_eq!(machine.detect(), None);
}

#[test]
fn the_minimum_driver_major_is_enough() {
    let machine = FakeMachine::nvidia_only("minimum-driver");
    let minimum = format!("{MIN_DRIVER_MAJOR}.45.04");
    machine.write(
        "proc/driver/nvidia/version",
        &OPEN_MODULE_615.replace("615.78.08", &minimum),
    );
    let support = machine.detect().unwrap();
    assert_eq!(support.driver_version, minimum);
}

#[test]
fn modeset_off_has_no_venus() {
    let machine = FakeMachine::nvidia_only("modeset-off");
    machine.write("sys/module/nvidia_drm/parameters/modeset", "N\n");
    assert_eq!(machine.detect(), None);
}

#[test]
fn a_missing_udmabuf_has_no_venus() {
    let machine = FakeMachine::nvidia_only("no-udmabuf");
    machine.remove("sys/class/misc/udmabuf");
    assert_eq!(machine.detect(), None);
}

#[test]
fn an_unloaded_nvidia_module_has_no_venus() {
    let machine = FakeMachine::nvidia_only("no-module");
    machine.remove("proc/driver/nvidia");
    assert_eq!(machine.detect(), None);
}

#[test]
fn a_machine_without_drm_has_no_venus() {
    let machine = FakeMachine::nvidia_only("no-drm");
    machine.remove("sys/class/drm");
    assert_eq!(machine.detect(), None);
}

#[test]
fn without_a_boot_vga_card_the_first_card_with_a_render_node_is_the_compositor_gpu() {
    let machine = FakeMachine::nvidia_only("no-boot-vga");
    machine.remove("sys/class/drm/card0");
    machine.gpu("card0", "nvidia", None, "renderD128");
    machine.gpu("card1", "amdgpu", None, "renderD129");
    assert_eq!(machine.detect(), supported_on("/dev/dri/renderD128"));
}
