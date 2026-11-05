package org.crownos.agent.task;

import android.app.ActivityManager;
import android.os.IBinder;

import java.util.List;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.Services;

/** {@code IActivityTaskManager.getTasks} over every display. */
final class TaskQueries {
    private static final int MAX_TASKS = 100;
    private static final int ALL_DISPLAYS = -1;

    private TaskQueries() {
    }

    @SuppressWarnings("unchecked")
    static List<ActivityManager.RunningTaskInfo> running() {
        return (List<ActivityManager.RunningTaskInfo>) Hidden.call(Services.activityTaskManager(), "getTasks",
                Hidden.types(int.class, boolean.class, boolean.class, int.class),
                MAX_TASKS, false, false, ALL_DISPLAYS);
    }

    static boolean isRunning(int taskId) {
        for (ActivityManager.RunningTaskInfo info : running()) {
            if (info.taskId == taskId) {
                return true;
            }
        }
        return false;
    }

    /** The task holding {@code cookie}, or -1. */
    static int withLaunchCookie(IBinder cookie) {
        for (ActivityManager.RunningTaskInfo info : running()) {
            if (TaskSnapshot.of(info).hasLaunchCookie(cookie)) {
                return info.taskId;
            }
        }
        return -1;
    }
}
