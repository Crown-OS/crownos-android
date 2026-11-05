package org.crownos.agent;

import android.net.LocalServerSocket;
import android.os.Process;
import android.os.SystemClock;
import android.system.ErrnoException;

import java.io.IOException;
import java.util.function.IntPredicate;

import org.crownos.agent.link.DaemonSession;
import org.crownos.agent.link.HwcSession;
import org.crownos.agent.link.PeerLink;
import org.crownos.agent.link.Sockets;
import org.crownos.agent.platform.Services;
import org.crownos.agent.platform.SystemProps;
import org.crownos.agent.policy.InboxPolicy;
import org.crownos.agent.system.BootClock;
import org.crownos.agent.system.BootSettings;
import org.crownos.agent.system.HostUser;
import org.crownos.agent.system.keepalive.KeepAliveSignals;
import org.crownos.agent.task.TaskCoordinator;
import org.crownos.agent.task.TaskPlacement;
import org.crownos.agent.task.TaskSource;
import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Loop;
import org.crownos.agent.wire.RecordSink;

/** Wires the sockets, the task coordinator and the boot sequence together. */
final class Agent {
    private static final String HWC_SOCKET = "crown_agent";
    private static final String DATA_DIRECTORY = "/data/crownos";
    private static final String DAEMON_SOCKET = DATA_DIRECTORY + "/agent.sock";
    /** The fixed Android display the atlas packs (contract 11). */
    private static final int DEFAULT_DISPLAY_SIZE = 4096;
    private static final long BOOT_POLL_MILLIS = 250;
    private static final int SYSTEM_UID = 1000;

    private final Loop loop = new Loop("crown-agent");
    private final boolean safeMode = SystemProps.isOn("persist.crownos.safe_mode");
    private final String bundle = SystemProps.get("ro.crownos.bundle", "");
    private final HostUser host = HostUser.fromProperties();

    void run() {
        BootSettings.applyActiveApps();
        PeerLink hwc = link("hwc", hwcSocket(), uid -> uid == Process.ROOT_UID || uid == SYSTEM_UID);
        PeerLink daemon = link("daemon", daemonSocket(), host::is);
        TaskCoordinator coordinator = new TaskCoordinator(loop,
                SystemProps.getInt("persist.waydroid.width", DEFAULT_DISPLAY_SIZE),
                SystemProps.getInt("persist.waydroid.height", DEFAULT_DISPLAY_SIZE),
                sink(hwc), sink(daemon));
        DaemonSession daemonSession = new DaemonSession(coordinator, sink(daemon), KeepAliveSignals.probe());
        if (hwc != null) {
            hwc.start(new HwcSession(coordinator));
        }
        if (daemon != null) {
            daemon.start(daemonSession);
        }
        AgentLog.info("started" + (safeMode ? " in safe mode" : "") + ", bundle " + bundle + ", host uid "
                + host.uid);
        Services.awaitSystemServer();
        while (!SystemProps.isOn("sys.boot_completed")) {
            SystemClock.sleep(BOOT_POLL_MILLIS);
        }
        loop.execute(() -> finishBoot(coordinator, daemonSession));
    }

    private void finishBoot(TaskCoordinator coordinator, DaemonSession daemonSession) {
        TaskSource source = new TaskSource(loop, coordinator);
        if (safeMode) {
            source.observe();
        } else {
            BootSettings.apply();
            if (source.organize() == TaskPlacement.OBSERVED) {
                AgentLog.warn("tasks keep their Android decorations: organizer mode is unavailable");
            }
        }
        daemonSession.markReady(BootClock.millisSinceInit());
    }

    private PeerLink link(String name, LocalServerSocket socket, IntPredicate admitsUid) {
        return socket == null ? null : new PeerLink(name, socket, admitsUid, bundle, loop);
    }

    private static RecordSink sink(PeerLink link) {
        return link == null ? record -> { } : link;
    }

    private static LocalServerSocket hwcSocket() {
        try {
            return Sockets.inherited(HWC_SOCKET);
        } catch (IOException e) {
            AgentLog.error("no hwcomposer socket; host windows get no task control", e);
            return null;
        }
    }

    private LocalServerSocket daemonSocket() {
        try {
            host.ownDirectory(DATA_DIRECTORY);
            host.ownDirectory(InboxPolicy.INBOX_DIRECTORY);
            return Sockets.ownedBy(DAEMON_SOCKET, host.uid, host.gid);
        } catch (ErrnoException | IOException e) {
            AgentLog.error("no daemon socket at " + DAEMON_SOCKET, e);
            return null;
        }
    }
}
