package android.window;

import android.app.ActivityManager;
import android.view.SurfaceControl;

import java.util.List;

public class TaskOrganizer {
    public TaskOrganizer() {
        throw new UnsupportedOperationException("compile-time stub");
    }

    public List<?> registerOrganizer() {
        throw new UnsupportedOperationException("compile-time stub");
    }

    public void unregisterOrganizer() {
    }

    public void onTaskAppeared(ActivityManager.RunningTaskInfo taskInfo, SurfaceControl leash) {
    }

    public void onTaskInfoChanged(ActivityManager.RunningTaskInfo taskInfo) {
    }

    public void onTaskVanished(ActivityManager.RunningTaskInfo taskInfo) {
    }
}
