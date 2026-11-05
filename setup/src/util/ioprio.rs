use std::io;

const IOPRIO_WHO_PROCESS: libc::c_int = 1;
const IOPRIO_CLASS_IDLE: libc::c_int = 3;
const IOPRIO_CLASS_SHIFT: libc::c_int = 13;
const THIS_PROCESS: libc::c_int = 0;

/// Moves this process and the children it starts into the idle IO class, so
/// image downloads and unpacking never compete with the desktop for the disk.
pub fn lower_to_idle() -> io::Result<()> {
    // SAFETY: ioprio_set takes three integers and touches no memory.
    let result = unsafe {
        libc::syscall(
            libc::SYS_ioprio_set,
            IOPRIO_WHO_PROCESS,
            THIS_PROCESS,
            IOPRIO_CLASS_IDLE << IOPRIO_CLASS_SHIFT,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
