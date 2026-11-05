package org.crownos.agent;

import android.os.IBinder;

final class Services {
    static final String ACTIVITY_TASK_MANAGER = "android.app.IActivityTaskManager";
    static final String CALLER_PACKAGE = "com.android.shell";
    static final int USER_SYSTEM = 0;

    private static final long SERVICE_POLL_MS = 200;
    private static final long SERVICE_WAIT_LIMIT_MS = 60_000;

    private static Object activityTaskManager;
    private static Object windowOrganizerController;
    private static Object packageManager;
    private static Object notificationManager;

    private Services() {
    }

    static synchronized Object activityTaskManager() {
        if (activityTaskManager == null) {
            activityTaskManager = asInterface(ACTIVITY_TASK_MANAGER, awaitBinder("activity_task"));
        }
        return activityTaskManager;
    }

    static synchronized Object windowOrganizerController() {
        if (windowOrganizerController == null) {
            windowOrganizerController = Hidden.call(activityTaskManager(), "getWindowOrganizerController",
                    Hidden.NO_TYPES);
        }
        return windowOrganizerController;
    }

    static synchronized Object packageManager() {
        if (packageManager == null) {
            packageManager = asInterface("android.content.pm.IPackageManager", awaitBinder("package"));
        }
        return packageManager;
    }

    static synchronized Object notificationManager() {
        if (notificationManager == null) {
            notificationManager = asInterface("android.app.INotificationManager", awaitBinder("notification"));
        }
        return notificationManager;
    }

    private static Object asInterface(String interfaceName, IBinder binder) {
        return Hidden.callStatic(Hidden.type(interfaceName + "$Stub"), "asInterface", new Class<?>[] {IBinder.class},
                binder);
    }

    private static IBinder awaitBinder(String serviceName) {
        Class<?> serviceManager = Hidden.type("android.os.ServiceManager");
        long deadline = System.nanoTime() + SERVICE_WAIT_LIMIT_MS * 1_000_000L;
        while (true) {
            IBinder binder = (IBinder) Hidden.callStatic(serviceManager, "getService", new Class<?>[] {String.class},
                    serviceName);
            if (binder != null) {
                return binder;
            }
            if (System.nanoTime() > deadline) {
                throw new IllegalStateException("service not available: " + serviceName);
            }
            Lifetime.sleep(SERVICE_POLL_MS);
        }
    }
}
