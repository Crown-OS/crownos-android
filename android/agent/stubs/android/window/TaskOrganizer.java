package android.window;

import android.app.ActivityManager;
import android.view.SurfaceControl;

/** Compile-only copy of the hidden Android 13 class; the device's own class is used at runtime. */
public class TaskOrganizer {
    public TaskOrganizer() {
        throw new UnsupportedOperationException("compile-time stub");
    }

    public void onTaskAppeared(ActivityManager.RunningTaskInfo taskInfo, SurfaceControl leash) {
    }

    public void onTaskInfoChanged(ActivityManager.RunningTaskInfo taskInfo) {
    }

    public void onTaskVanished(ActivityManager.RunningTaskInfo taskInfo) {
    }
}
