package org.crownos.agent;

import android.os.Binder;
import android.os.IBinder;
import android.system.Os;

final class Toaster {
    private static final int ROOT_UID = 0;
    private static final String ROOT_PACKAGE = "root";
    private static final int LENGTH_LONG = 1;
    private static final int DEFAULT_DISPLAY = 0;
    private static final long TOAST_VISIBLE_MS = 4000;

    private Toaster() {
    }

    static void show(String text) {
        String callerPackage = Os.getuid() == ROOT_UID ? ROOT_PACKAGE : Services.CALLER_PACKAGE;
        IBinder token = new Binder();
        Hidden.call(Services.notificationManager(), "enqueueTextToast",
                new Class<?>[] {String.class, IBinder.class, CharSequence.class, int.class, int.class,
                    Hidden.type("android.app.ITransientNotificationCallback")},
                callerPackage, token, text, LENGTH_LONG, DEFAULT_DISPLAY, null);
        Events.emit("toast", "package=" + callerPackage + " text=\"" + text + "\"");
        Lifetime.sleep(TOAST_VISIBLE_MS);
    }
}
