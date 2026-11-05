package org.crownos.agent;

import android.app.ActivityManager;
import android.app.TaskStackListener;
import android.content.ComponentName;

final class TaskStackWatcher extends TaskStackListener {
    private TaskStackWatcher() {
    }

    static void watch(int seconds) {
        TaskStackWatcher watcher = new TaskStackWatcher();
        Hidden.call(Services.activityTaskManager(), "registerTaskStackListener",
                new Class<?>[] {Hidden.type("android.app.ITaskStackListener")}, watcher);
        Events.emit("watch.started", "seconds=" + seconds);
        Lifetime.hold(seconds);
        Hidden.call(Services.activityTaskManager(), "unregisterTaskStackListener",
                new Class<?>[] {Hidden.type("android.app.ITaskStackListener")}, watcher);
        Events.emit("watch.stopped", "");
    }

    @Override
    public void onTaskStackChanged() {
        Events.emit("stack.changed", "");
    }

    @Override
    public void onTaskCreated(int taskId, ComponentName componentName) {
        Events.emit("stack.created", "task=" + taskId + " component="
                + (componentName == null ? "none" : componentName.flattenToShortString()));
    }

    @Override
    public void onTaskRemoved(int taskId) {
        Events.emit("stack.removed", "task=" + taskId);
    }

    @Override
    public void onTaskRemovalStarted(ActivityManager.RunningTaskInfo taskInfo) {
        Events.emit("stack.removal_started", "task=" + taskInfo.taskId);
    }

    @Override
    public void onTaskMovedToFront(ActivityManager.RunningTaskInfo taskInfo) {
        Events.emit("stack.front", TaskRecord.of(taskInfo).describe());
    }

    @Override
    public void onTaskFocusChanged(int taskId, boolean focused) {
        Events.emit("stack.focus", "task=" + taskId + " focused=" + focused);
    }

    @Override
    public void onTaskDescriptionChanged(ActivityManager.RunningTaskInfo taskInfo) {
        Events.emit("stack.description", TaskRecord.of(taskInfo).describe());
    }

    @Override
    public void onTaskDisplayChanged(int taskId, int newDisplayId) {
        Events.emit("stack.display", "task=" + taskId + " display=" + newDisplayId);
    }

    @Override
    public void onTaskRequestedOrientationChanged(int taskId, int requestedOrientation) {
        Events.emit("stack.orientation", "task=" + taskId + " orientation=" + requestedOrientation);
    }

    @Override
    public void onActivityRestartAttempt(ActivityManager.RunningTaskInfo taskInfo, boolean homeTaskVisible,
            boolean clearedTask, boolean wasVisible) {
        Events.emit("stack.restart_attempt", "task=" + taskInfo.taskId + " was_visible=" + wasVisible);
    }

    @Override
    public void onBackPressedOnTaskRoot(ActivityManager.RunningTaskInfo taskInfo) {
        Events.emit("stack.back_on_root", "task=" + taskInfo.taskId);
    }

    @Override
    public void onActivityPinned(String packageName, int userId, int taskId, int rootTaskId) {
        Events.emit("stack.pinned", "task=" + taskId + " package=" + packageName);
    }

    @Override
    public void onActivityUnpinned() {
        Events.emit("stack.unpinned", "");
    }
}
