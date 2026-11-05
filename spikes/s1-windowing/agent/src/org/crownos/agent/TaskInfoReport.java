package org.crownos.agent;

import android.app.ActivityManager;
import android.content.pm.ActivityInfo;

final class TaskInfoReport {
    private TaskInfoReport() {
    }

    static void print(int taskId) {
        ActivityManager.RunningTaskInfo info = TaskQueries.require(taskId);
        TaskRecord task = TaskRecord.of(info);
        ActivityInfo topActivity = (ActivityInfo) Hidden.field(info, "topActivityInfo");
        ActivityInfo.WindowLayout layout = topActivity == null ? null : topActivity.windowLayout;
        Events.emit("info", task.describe()
                + " app_label=\"" + AppLabel.of(task.packageName()) + "\""
                + " manifest_min=" + (layout == null ? "none" : layout.minWidth + "x" + layout.minHeight)
                + " orientation=" + (topActivity == null ? "none" : String.valueOf(topActivity.screenOrientation)));
    }
}
