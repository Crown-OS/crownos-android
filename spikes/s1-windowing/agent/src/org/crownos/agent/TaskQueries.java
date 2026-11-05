package org.crownos.agent;

import android.app.ActivityManager;

import java.util.List;

final class TaskQueries {
    private static final int MAX_TASKS = 64;
    private static final int ALL_DISPLAYS = -1;

    private TaskQueries() {
    }

    @SuppressWarnings("unchecked")
    static List<ActivityManager.RunningTaskInfo> running() {
        return (List<ActivityManager.RunningTaskInfo>) Hidden.call(Services.activityTaskManager(), "getTasks",
                new Class<?>[] {int.class, boolean.class, boolean.class, int.class},
                MAX_TASKS, false, false, ALL_DISPLAYS);
    }

    static ActivityManager.RunningTaskInfo find(int taskId) {
        for (ActivityManager.RunningTaskInfo info : running()) {
            if (info.taskId == taskId) {
                return info;
            }
        }
        return null;
    }

    static ActivityManager.RunningTaskInfo require(int taskId) {
        ActivityManager.RunningTaskInfo info = find(taskId);
        if (info == null) {
            throw new IllegalArgumentException("no running task " + taskId);
        }
        return info;
    }

    static void list() {
        List<ActivityManager.RunningTaskInfo> tasks = running();
        for (ActivityManager.RunningTaskInfo info : tasks) {
            Events.emit("task", TaskRecord.of(info).describe());
        }
        Events.emit("tasks.count", "count=" + tasks.size());
    }
}
