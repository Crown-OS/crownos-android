package org.crownos.agent.platform;

import android.os.IBinder;
import android.os.RemoteException;
import android.os.SystemClock;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import org.crownos.agent.util.AgentLog;

/** Binder interfaces of system services, looked up once through the hidden ServiceManager. */
public final class Services {
    public static final int USER_SYSTEM = 0;
    /** Calling package for task and activity calls; root passes every package check (S1 d.). */
    public static final String SHELL_PACKAGE = "com.android.shell";
    /** Calling package AppOps resolves to uid 0. */
    public static final String ROOT_PACKAGE = "root";

    private static final long POLL_MILLIS = 200;
    private static final long REQUIRED_WAIT_MILLIS = 10_000;
    private static final long PROGRESS_LOG_MILLIS = 30_000;
    private static final int EXIT_SYSTEM_SERVER_DIED = 1;
    private static final Map<String, Object> INTERFACES = new ConcurrentHashMap<>();

    private Services() {
    }

    /** Blocks until system_server publishes its task service, and exits the agent when it dies. */
    public static void awaitSystemServer() {
        long started = SystemClock.elapsedRealtime();
        long nextReport = started + PROGRESS_LOG_MILLIS;
        IBinder binder;
        while ((binder = lookup("activity_task")) == null) {
            if (SystemClock.elapsedRealtime() > nextReport) {
                AgentLog.info("still waiting for system_server");
                nextReport += PROGRESS_LOG_MILLIS;
            }
            SystemClock.sleep(POLL_MILLIS);
        }
        try {
            binder.linkToDeath(() -> {
                AgentLog.warn("system_server died; exiting so init restarts the agent");
                System.exit(EXIT_SYSTEM_SERVER_DIED);
            }, 0);
        } catch (RemoteException e) {
            System.exit(EXIT_SYSTEM_SERVER_DIED);
        }
    }

    public static Object activityTaskManager() {
        return require("activity_task", "android.app.IActivityTaskManager");
    }

    public static Object activityManager() {
        return require("activity", "android.app.IActivityManager");
    }

    public static Object packageManager() {
        return require("package", "android.content.pm.IPackageManager");
    }

    public static Object inputManager() {
        return require("input", "android.hardware.input.IInputManager");
    }

    public static Object audioService() {
        return require("audio", "android.media.IAudioService");
    }

    public static Object notificationManager() {
        return require("notification", "android.app.INotificationManager");
    }

    /** The telecom service, or {@code null} on images without one. */
    public static Object telecomService() {
        return optional("telecom", "com.android.internal.telecom.ITelecomService");
    }

    public static Object roleManager() {
        return require("role", "android.app.role.IRoleManager");
    }

    private static Object require(String serviceName, String interfaceName) {
        Object cached = INTERFACES.get(serviceName);
        if (cached != null) {
            return cached;
        }
        long deadline = SystemClock.elapsedRealtime() + REQUIRED_WAIT_MILLIS;
        IBinder binder;
        while ((binder = lookup(serviceName)) == null) {
            if (SystemClock.elapsedRealtime() > deadline) {
                throw new HiddenApiException("service not available: " + serviceName, null);
            }
            SystemClock.sleep(POLL_MILLIS);
        }
        return asInterface(serviceName, interfaceName, binder);
    }

    private static Object optional(String serviceName, String interfaceName) {
        Object cached = INTERFACES.get(serviceName);
        if (cached != null) {
            return cached;
        }
        IBinder binder = lookup(serviceName);
        return binder == null ? null : asInterface(serviceName, interfaceName, binder);
    }

    private static Object asInterface(String serviceName, String interfaceName, IBinder binder) {
        Object service = Hidden.callStatic(Hidden.type(interfaceName + "$Stub"), "asInterface",
                Hidden.types(IBinder.class), binder);
        INTERFACES.put(serviceName, service);
        return service;
    }

    private static IBinder lookup(String serviceName) {
        return (IBinder) Hidden.callStatic(Hidden.type("android.os.ServiceManager"), "checkService",
                Hidden.types(String.class), serviceName);
    }
}
