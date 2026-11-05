package org.crownos.agent;

import android.app.ActivityManager;
import android.app.ActivityOptions;
import android.content.ComponentName;
import android.content.Intent;
import android.content.pm.ResolveInfo;
import android.graphics.Rect;
import android.os.Binder;
import android.os.Bundle;
import android.os.IBinder;

import java.util.List;

final class Launch {
    private static final long TASK_POLL_MS = 10;
    private static final long TASK_WAIT_LIMIT_MS = 5000;

    private Launch() {
    }

    static void start(String target, Rect bounds, WindowingMode mode) {
        ComponentName component = resolve(target);
        Intent intent = new Intent(Intent.ACTION_MAIN)
                .addCategory(Intent.CATEGORY_LAUNCHER)
                .setComponent(component)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        IBinder launchCookie = new Binder();
        ActivityOptions options = ActivityOptions.makeBasic().setLaunchBounds(bounds);
        Hidden.call(options, "setLaunchWindowingMode", new Class<?>[] {int.class}, mode.value);
        Hidden.call(options, "setLaunchCookie", new Class<?>[] {IBinder.class}, launchCookie);

        long commandNanos = Events.monotonicNanos();
        int result = (Integer) Hidden.call(Services.activityTaskManager(), "startActivityAsUser",
                new Class<?>[] {
                    Hidden.type("android.app.IApplicationThread"), String.class, String.class, Intent.class,
                    String.class, IBinder.class, String.class, int.class, int.class,
                    Hidden.type("android.app.ProfilerInfo"), Bundle.class, int.class},
                null, Services.CALLER_PACKAGE, null, intent, null, null, null, 0, 0, null, options.toBundle(),
                Services.USER_SYSTEM);
        Events.emit("launch.result", "code=" + result + " component=" + component.flattenToShortString()
                + " mode=" + mode.label + " bounds=" + Rects.format(bounds) + " t_cmd=" + commandNanos);

        ActivityManager.RunningTaskInfo task = awaitTask(launchCookie);
        Events.emit("launch.task", task == null ? "status=timeout" : TaskRecord.of(task).describe());
    }

    private static ComponentName resolve(String target) {
        if (target.contains("/")) {
            ComponentName component = ComponentName.unflattenFromString(target);
            if (component == null) {
                throw new IllegalArgumentException("bad component " + target);
            }
            return component;
        }
        Intent query = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER).setPackage(target);
        Object slice = Hidden.call(Services.packageManager(), "queryIntentActivities",
                new Class<?>[] {Intent.class, String.class, long.class, int.class},
                query, null, 0L, Services.USER_SYSTEM);
        List<?> matches = (List<?>) Hidden.call(slice, "getList", Hidden.NO_TYPES);
        if (matches.isEmpty()) {
            throw new IllegalArgumentException("no launcher activity in " + target);
        }
        ResolveInfo first = (ResolveInfo) matches.get(0);
        return new ComponentName(first.activityInfo.packageName, first.activityInfo.name);
    }

    private static ActivityManager.RunningTaskInfo awaitTask(IBinder launchCookie) {
        long deadline = System.nanoTime() + TASK_WAIT_LIMIT_MS * 1_000_000L;
        while (System.nanoTime() < deadline) {
            for (ActivityManager.RunningTaskInfo info : TaskQueries.running()) {
                if (TaskRecord.of(info).launchCookies.contains(launchCookie)) {
                    return info;
                }
            }
            Lifetime.sleep(TASK_POLL_MS);
        }
        return null;
    }
}
