package org.crownos.agent.system;

import android.content.IntentSender;
import android.content.pm.PackageInstaller;
import android.os.ParcelFileDescriptor;
import android.system.ErrnoException;
import android.system.Int64Ref;
import android.system.Os;
import android.system.OsConstants;
import android.system.StructStat;

import java.io.FileDescriptor;
import java.io.IOException;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.util.AgentLog;

/**
 * Installs one APK from the inbox through a PackageInstaller session. The file is opened with
 * O_NOFOLLOW and must be a regular file, so a symlink planted in the user-owned inbox can never make
 * root install something else. The inbox file is removed afterwards either way.
 */
public final class PackageInstall {
    private static final String SESSION_FILE = "base.apk";
    private static final long COMMIT_WAIT_MINUTES = 10;

    private PackageInstall() {
    }

    public static InstallOutcome install(String path) {
        FileDescriptor apk;
        try {
            apk = Os.open(path, OsConstants.O_RDONLY | OsConstants.O_NOFOLLOW, 0);
        } catch (ErrnoException e) {
            return InstallOutcome.failed("cannot open " + path + ": " + e.getMessage());
        }
        try {
            StructStat status = Os.fstat(apk);
            if (!OsConstants.S_ISREG(status.st_mode)) {
                return InstallOutcome.failed(path + " is not a regular file");
            }
            return commit(apk, status.st_size);
        } catch (ErrnoException e) {
            return InstallOutcome.failed("cannot read " + path + ": " + e.getMessage());
        } finally {
            closeQuietly(apk);
            removeQuietly(path);
        }
    }

    private static InstallOutcome commit(FileDescriptor apk, long size) {
        Object installer;
        int sessionId;
        try {
            installer = Hidden.call(Services.packageManager(), "getPackageInstaller", Hidden.NO_TYPES);
            PackageInstaller.SessionParams params =
                    new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
            params.setSize(size);
            sessionId = (Integer) Hidden.call(installer, "createSession",
                    Hidden.types(PackageInstaller.SessionParams.class, String.class, String.class, int.class),
                    params, Services.SHELL_PACKAGE, null, Services.USER_SYSTEM);
        } catch (HiddenApiException e) {
            return InstallOutcome.failed("cannot create an install session: " + e.getMessage());
        }
        Object session = null;
        try {
            session = Hidden.call(installer, "openSession", Hidden.types(int.class), sessionId);
            copy(session, apk, size);
            InstallStatusReceiver receiver = new InstallStatusReceiver();
            IntentSender statusSender = (IntentSender) Hidden.construct(IntentSender.class,
                    Hidden.types("android.content.IIntentSender"), receiver);
            Hidden.call(session, "commit", Hidden.types(IntentSender.class, boolean.class), statusSender, false);
            return receiver.await(COMMIT_WAIT_MINUTES);
        } catch (HiddenApiException | ErrnoException | IOException | LinkageError e) {
            abandon(installer, sessionId);
            return InstallOutcome.failed("install session failed: " + e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            abandon(installer, sessionId);
            return InstallOutcome.failed("interrupted");
        } finally {
            closeSession(session);
        }
    }

    /** Copies the APK into the session file in the kernel with sendfile. */
    private static void copy(Object session, FileDescriptor apk, long size) throws ErrnoException, IOException {
        ParcelFileDescriptor target = (ParcelFileDescriptor) Hidden.call(session, "openWrite",
                Hidden.types(String.class, long.class, long.class), SESSION_FILE, 0L, size);
        try {
            Int64Ref offset = new Int64Ref(0);
            while (offset.value < size) {
                if (Os.sendfile(target.getFileDescriptor(), apk, offset, size - offset.value) == 0) {
                    throw new IOException("the APK shrank while it was copied");
                }
            }
        } finally {
            target.close();
        }
    }

    private static void abandon(Object installer, int sessionId) {
        try {
            Hidden.call(installer, "abandonSession", Hidden.types(int.class), sessionId);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot abandon install session " + sessionId + ": " + e.getMessage());
        }
    }

    private static void closeSession(Object session) {
        if (session == null) {
            return;
        }
        try {
            Hidden.call(session, "close", Hidden.NO_TYPES);
        } catch (HiddenApiException e) {
            AgentLog.info("install session already closed: " + e.getMessage());
        }
    }

    private static void closeQuietly(FileDescriptor descriptor) {
        try {
            Os.close(descriptor);
        } catch (ErrnoException e) {
            AgentLog.warn("cannot close an inbox file: " + e.getMessage());
        }
    }

    private static void removeQuietly(String path) {
        try {
            Os.remove(path);
        } catch (ErrnoException e) {
            AgentLog.warn("cannot remove " + path + ": " + e.getMessage());
        }
    }
}
