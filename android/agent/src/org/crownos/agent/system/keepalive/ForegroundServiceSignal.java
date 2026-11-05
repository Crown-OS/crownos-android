package org.crownos.agent.system.keepalive;

import android.app.Notification;
import android.service.notification.StatusBarNotification;

import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

import org.crownos.agent.keepalive.KeepAliveReasons;
import org.crownos.agent.keepalive.KeepAliveSignal;
import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.Services;

/** An app runs a foreground service with its ongoing notification, such as navigation or a music player. */
final class ForegroundServiceSignal implements KeepAliveSignal {
    private static final int ONGOING_FOREGROUND_SERVICE =
            Notification.FLAG_FOREGROUND_SERVICE | Notification.FLAG_ONGOING_EVENT;
    /** System services that run permanently; they are not a reason to keep Android up. */
    private static final Set<String> SYSTEM_PACKAGES = new HashSet<>(Arrays.asList("android", "com.android.systemui"));

    @Override
    public int reason() {
        return KeepAliveReasons.FOREGROUND_SERVICE;
    }

    @Override
    public String name() {
        return "foreground service";
    }

    @Override
    public boolean isActive() {
        StatusBarNotification[] notifications = (StatusBarNotification[]) Hidden.call(Services.notificationManager(),
                "getActiveNotificationsWithAttribution", Hidden.types(String.class, String.class),
                Services.ROOT_PACKAGE, null);
        for (StatusBarNotification notification : notifications) {
            if ((notification.getNotification().flags & ONGOING_FOREGROUND_SERVICE) == ONGOING_FOREGROUND_SERVICE
                    && !SYSTEM_PACKAGES.contains(notification.getPackageName())) {
                return true;
            }
        }
        return false;
    }
}
