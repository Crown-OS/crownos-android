package org.crownos.agent.task;

import android.app.ActivityManager;
import android.view.SurfaceControl;
import android.window.TaskOrganizer;

import java.util.List;
import java.util.concurrent.Executor;

import org.crownos.agent.platform.Hidden;

/**
 * The agent's TaskOrganizer. Android 13 hands every root task, in every windowing mode, to the most
 * recently registered organizer. It creates no starting windows: the host shows the splash.
 */
final class TaskOrganizerBridge extends TaskOrganizer {
    private final Executor loop;
    private final TaskEvents events;

    TaskOrganizerBridge(Executor loop, TaskEvents events) {
        this.loop = loop;
        this.events = events;
    }

    /** Registers and returns the {@code TaskAppearedInfo} of every task it took over. */
    List<?> register() {
        return (List<?>) Hidden.call(this, "registerOrganizer", Hidden.NO_TYPES);
    }

    void unregister() {
        Hidden.call(this, "unregisterOrganizer", Hidden.NO_TYPES);
    }

    @Override
    public void onTaskAppeared(ActivityManager.RunningTaskInfo taskInfo, SurfaceControl leash) {
        loop.execute(() -> events.onAppeared(taskInfo, leash));
    }

    @Override
    public void onTaskInfoChanged(ActivityManager.RunningTaskInfo taskInfo) {
        loop.execute(() -> events.onChanged(taskInfo));
    }

    @Override
    public void onTaskVanished(ActivityManager.RunningTaskInfo taskInfo) {
        loop.execute(() -> events.onVanished(taskInfo));
    }
}
