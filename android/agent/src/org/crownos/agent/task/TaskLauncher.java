package org.crownos.agent.task;

import android.app.ActivityOptions;
import android.content.ComponentName;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.ResolveInfo;
import android.graphics.Rect;
import android.net.Uri;
import android.os.Bundle;
import android.os.IBinder;

import java.util.ArrayList;
import java.util.List;

import org.crownos.agent.atlas.Atlas;
import org.crownos.agent.atlas.Slot;
import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.policy.PackageNames;
import org.crownos.agent.util.Loop;

/**
 * Starts activities FREEFORM with launch bounds in a reserved atlas slot, and pairs each start with
 * the task it produces, new or reused, through a launch cookie.
 */
final class TaskLauncher {
    /** Hears how launches end. */
    interface Listener {
        /** Answers whether {@code taskId} is already a window; the launch is then complete. */
        boolean onLaunchResolved(PendingLaunch launch, int taskId);

        void onLaunchFailed(PendingLaunch launch, String reason);

        void onLaunchRefused(String packageName, String reason);
    }

    private static final int DEFAULT_WIDTH = 1280;
    private static final int DEFAULT_HEIGHT = 800;
    private static final long TASK_WAIT_MILLIS = 10_000;
    private static final int START_SWITCHES_CANCELED = 4;
    private static final int START_ABORTED = 102;
    private static final int DEFAULT_DISPLAY = 0;
    private static final int LAUNCHER_FLAGS = Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED;

    private final Loop loop;
    private final Atlas atlas;
    private final Listener listener;
    private final List<PendingLaunch> pending = new ArrayList<>();
    private int nextReservation = -1;
    private int defaultWidth = DEFAULT_WIDTH;
    private int defaultHeight = DEFAULT_HEIGHT;

    TaskLauncher(Loop loop, Atlas atlas, Listener listener) {
        this.loop = loop;
        this.atlas = atlas;
        this.listener = listener;
    }

    /** The size for tasks the daemon did not size: the most recent Launch size. */
    int defaultWidth() {
        return defaultWidth;
    }

    int defaultHeight() {
        return defaultHeight;
    }

    void launch(String packageName, int width, int height, String activationToken) {
        if (!PackageNames.isValid(packageName)) {
            listener.onLaunchRefused(packageName, "invalid package name");
            return;
        }
        ComponentName component;
        try {
            component = launcherActivity(packageName);
        } catch (HiddenApiException e) {
            listener.onLaunchRefused(packageName, "package query failed: " + e.getMessage());
            return;
        }
        if (component == null) {
            listener.onLaunchRefused(packageName, isInstalled(packageName) ? "no launcher activity" : "not installed");
            return;
        }
        if (width > 0 && height > 0) {
            defaultWidth = width;
            defaultHeight = height;
        }
        Intent intent = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
                .setComponent(component).addFlags(LAUNCHER_FLAGS);
        start(new PendingLaunch(packageName, activationToken, reserve(defaultWidth, defaultHeight)), intent);
    }

    /** Opens a link the caller already checked with {@code UriPolicy}. */
    void openUri(String uri) {
        Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(uri).normalizeScheme())
                .addCategory(Intent.CATEGORY_BROWSABLE).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        start(new PendingLaunch(null, "", reserve(defaultWidth, defaultHeight)), intent);
    }

    /** Takes the pending launch whose cookie reached {@code snapshot}. */
    PendingLaunch claimByCookie(TaskSnapshot snapshot) {
        for (PendingLaunch launch : pending) {
            if (snapshot.hasLaunchCookie(launch.cookie)) {
                return take(launch);
            }
        }
        return null;
    }

    /** Takes the pending launch that produced a new task, by cookie or else by package. */
    PendingLaunch claim(TaskSnapshot snapshot) {
        PendingLaunch byCookie = claimByCookie(snapshot);
        if (byCookie != null) {
            return byCookie;
        }
        for (PendingLaunch launch : pending) {
            if (launch.isFor(snapshot.packageName)) {
                return take(launch);
            }
        }
        return null;
    }

    private PendingLaunch take(PendingLaunch launch) {
        pending.remove(launch);
        if (launch.timeout != null) {
            launch.timeout.cancel(false);
        }
        return launch;
    }

    private int reserve(int width, int height) {
        int reservation = nextReservation--;
        atlas.place(reservation, width, height);
        return reservation;
    }

    private void start(PendingLaunch launch, Intent intent) {
        pending.add(launch);
        int result;
        try {
            result = startActivity(intent, launch);
        } catch (HiddenApiException e) {
            fail(launch, "start failed: " + e.getMessage());
            return;
        }
        if (result < 0 || result == START_SWITCHES_CANCELED || result == START_ABORTED) {
            fail(launch, "start failed with code " + result);
            return;
        }
        if (resolvedAlready(launch)) {
            take(launch);
            return;
        }
        launch.timeout = loop.schedule(() -> expire(launch), TASK_WAIT_MILLIS);
    }

    private void expire(PendingLaunch launch) {
        if (pending.remove(launch) && !resolvedAlready(launch)) {
            listener.onLaunchFailed(launch, "timed out waiting for its task");
        }
    }

    private boolean resolvedAlready(PendingLaunch launch) {
        try {
            int taskId = TaskQueries.withLaunchCookie(launch.cookie);
            return taskId >= 0 && listener.onLaunchResolved(launch, taskId);
        } catch (HiddenApiException e) {
            return false;
        }
    }

    private void fail(PendingLaunch launch, String reason) {
        take(launch);
        listener.onLaunchFailed(launch, reason);
    }

    private int startActivity(Intent intent, PendingLaunch launch) {
        Slot slot = atlas.slotOf(launch.reservation);
        ActivityOptions options = ActivityOptions.makeBasic()
                .setLaunchBounds(new Rect(slot.left, slot.top, slot.right(), slot.bottom()))
                .setLaunchDisplayId(DEFAULT_DISPLAY);
        Hidden.call(options, "setLaunchWindowingMode", Hidden.types(int.class), TaskSnapshot.WINDOWING_MODE_FREEFORM);
        Hidden.call(options, "setLaunchCookie", Hidden.types(IBinder.class), launch.cookie);
        return (Integer) Hidden.call(Services.activityTaskManager(), "startActivityAsUser",
                Hidden.types("android.app.IApplicationThread", String.class, String.class, Intent.class,
                        String.class, IBinder.class, String.class, int.class, int.class, "android.app.ProfilerInfo",
                        Bundle.class, int.class),
                null, Services.SHELL_PACKAGE, null, intent, null, null, null, 0, 0, null, options.toBundle(),
                Services.USER_SYSTEM);
    }

    private static ComponentName launcherActivity(String packageName) {
        Intent query = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER).setPackage(packageName);
        Object slice = Hidden.call(Services.packageManager(), "queryIntentActivities",
                Hidden.types(Intent.class, String.class, long.class, int.class), query, null, 0L,
                Services.USER_SYSTEM);
        List<?> matches = slice == null ? null : (List<?>) Hidden.call(slice, "getList", Hidden.NO_TYPES);
        if (matches == null || matches.isEmpty()) {
            return null;
        }
        ActivityInfo activity = ((ResolveInfo) matches.get(0)).activityInfo;
        return new ComponentName(activity.packageName, activity.name);
    }

    private static boolean isInstalled(String packageName) {
        try {
            return Hidden.call(Services.packageManager(), "getApplicationInfo",
                    Hidden.types(String.class, long.class, int.class), packageName, 0L, Services.USER_SYSTEM) != null;
        } catch (HiddenApiException e) {
            return false;
        }
    }
}
