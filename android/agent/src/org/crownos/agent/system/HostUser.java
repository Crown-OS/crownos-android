package org.crownos.agent.system;

import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;
import android.system.StructStat;

import org.crownos.agent.platform.SystemProps;

/**
 * The desktop user running the Waydroid session. Waydroid writes its ids into {@code waydroid.host.uid}
 * and {@code waydroid.host.gid} (tools/helpers/images.py {@code make_prop}); the vendor patch
 * "init: Define host user" reads the same property for the init user name {@code host}. The
 * container maps no uids, so a host process connects with exactly this uid.
 */
public final class HostUser {
    /** Init's default for {@code host} when the property is missing. */
    private static final int INIT_DEFAULT_UID = 1000;
    private static final int PRIVATE_DIRECTORY_MODE = 0700;

    public final int uid;
    public final int gid;

    private HostUser(int uid, int gid) {
        this.uid = uid;
        this.gid = gid;
    }

    public static HostUser fromProperties() {
        int uid = SystemProps.getInt("waydroid.host.uid", INIT_DEFAULT_UID);
        return new HostUser(uid, SystemProps.getInt("waydroid.host.gid", uid));
    }

    public boolean is(int peerUid) {
        return peerUid == uid;
    }

    /** Makes {@code path} a directory only this user can enter, replacing a file or symlink found there. */
    public void ownDirectory(String path) throws ErrnoException {
        StructStat status = lstatOrNull(path);
        if (status != null && !OsConstants.S_ISDIR(status.st_mode)) {
            Os.remove(path);
            status = null;
        }
        if (status == null) {
            Os.mkdir(path, PRIVATE_DIRECTORY_MODE);
        }
        Os.lchown(path, uid, gid);
        Os.chmod(path, PRIVATE_DIRECTORY_MODE);
    }

    private static StructStat lstatOrNull(String path) throws ErrnoException {
        try {
            return Os.lstat(path);
        } catch (ErrnoException e) {
            if (e.errno == OsConstants.ENOENT) {
                return null;
            }
            throw e;
        }
    }
}
