package org.crownos.agent.link;

import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.net.LocalSocketAddress;
import android.os.ParcelFileDescriptor;
import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/** Listening sockets: the one init creates for the HWC, and the filesystem socket for the daemon. */
public final class Sockets {
    /** Leaves only owner read and write on a socket file as bind creates it: mode 0600. */
    private static final int OWNER_ONLY_SOCKET_UMASK = 0177;
    private static final String INIT_SOCKET_PREFIX = "ANDROID_SOCKET_";
    /** Objects whose finalizers would close the listening descriptors. */
    private static final List<Object> KEEP_OPEN = new ArrayList<>();

    private Sockets() {
    }

    /** The socket init created from the {@code socket} line of the service. */
    public static synchronized LocalServerSocket inherited(String name) throws IOException {
        String descriptor = System.getenv(INIT_SOCKET_PREFIX + name);
        if (descriptor == null) {
            throw new IOException("init passed no socket " + name);
        }
        ParcelFileDescriptor file;
        try {
            file = ParcelFileDescriptor.adoptFd(Integer.parseInt(descriptor));
        } catch (NumberFormatException e) {
            throw new IOException("bad descriptor for socket " + name + ": " + descriptor, e);
        }
        KEEP_OPEN.add(file);
        return new LocalServerSocket(file.getFileDescriptor());
    }

    /**
     * Binds a stream socket at {@code path} with mode 0600, owned by {@code uid}:{@code gid}. The mode
     * comes from the umask at bind time and the owner from lchown, so a swapped-in symlink is never
     * followed.
     */
    public static synchronized LocalServerSocket ownedBy(String path, int uid, int gid) throws IOException {
        try {
            removeStale(path);
            LocalSocket binding = new LocalSocket(LocalSocket.SOCKET_STREAM);
            int previousUmask = Os.umask(OWNER_ONLY_SOCKET_UMASK);
            try {
                binding.bind(new LocalSocketAddress(path, LocalSocketAddress.Namespace.FILESYSTEM));
            } finally {
                Os.umask(previousUmask);
            }
            Os.lchown(path, uid, gid);
            KEEP_OPEN.add(binding);
            return new LocalServerSocket(binding.getFileDescriptor());
        } catch (ErrnoException e) {
            throw new IOException("cannot create " + path + ": " + e.getMessage(), e);
        }
    }

    private static void removeStale(String path) throws ErrnoException {
        try {
            Os.remove(path);
        } catch (ErrnoException e) {
            if (e.errno != OsConstants.ENOENT) {
                throw e;
            }
        }
    }
}
