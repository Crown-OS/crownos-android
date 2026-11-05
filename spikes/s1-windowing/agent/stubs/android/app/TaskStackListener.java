package android.app;

import android.content.ComponentName;

public class TaskStackListener {
    public TaskStackListener() {
        throw new UnsupportedOperationException("compile-time stub");
    }

    public void onTaskStackChanged() {
    }

    public void onTaskCreated(int taskId, ComponentName componentName) {
    }

    public void onTaskRemoved(int taskId) {
    }

    public void onTaskRemovalStarted(ActivityManager.RunningTaskInfo taskInfo) {
    }

    public void onTaskMovedToFront(ActivityManager.RunningTaskInfo taskInfo) {
    }

    public void onTaskFocusChanged(int taskId, boolean focused) {
    }

    public void onTaskDescriptionChanged(ActivityManager.RunningTaskInfo taskInfo) {
    }

    public void onTaskDisplayChanged(int taskId, int newDisplayId) {
    }

    public void onTaskRequestedOrientationChanged(int taskId, int requestedOrientation) {
    }

    public void onActivityRestartAttempt(ActivityManager.RunningTaskInfo taskInfo, boolean homeTaskVisible,
            boolean clearedTask, boolean wasVisible) {
    }

    public void onBackPressedOnTaskRoot(ActivityManager.RunningTaskInfo taskInfo) {
    }

    public void onActivityPinned(String packageName, int userId, int taskId, int rootTaskId) {
    }

    public void onActivityUnpinned() {
    }
}
