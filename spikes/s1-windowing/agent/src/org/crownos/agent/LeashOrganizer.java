package org.crownos.agent;

import android.app.ActivityManager;
import android.graphics.Rect;
import android.view.SurfaceControl;
import android.window.TaskOrganizer;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

final class LeashOrganizer extends TaskOrganizer {
    private static final long RECLAIM_DELAY_MS = 300;

    private final Map<Integer, SurfaceControl> leashes = new ConcurrentHashMap<>();
    private final ScheduledExecutorService reclaimer = Executors.newSingleThreadScheduledExecutor();
    private final AtomicBoolean reclaimPending = new AtomicBoolean();

    private LeashOrganizer() {
    }

    static void organize(int seconds) {
        LeashOrganizer organizer = new LeashOrganizer();
        organizer.takeOver();
        Lifetime.hold(seconds);
        organizer.unregisterOrganizer();
        organizer.reclaimer.shutdownNow();
        Events.emit("organizer.stopped", "");
    }

    @Override
    public void onTaskAppeared(ActivityManager.RunningTaskInfo taskInfo, SurfaceControl leash) {
        leashes.put(taskInfo.taskId, leash);
        TaskRecord task = TaskRecord.of(taskInfo);
        place(task, leash, true);
        Events.emit("organizer.appeared", task.describe());
    }

    @Override
    public void onTaskInfoChanged(ActivityManager.RunningTaskInfo taskInfo) {
        TaskRecord task = TaskRecord.of(taskInfo);
        SurfaceControl leash = leashes.get(task.taskId);
        if (leash != null) {
            place(task, leash, false);
        }
        Events.emit("organizer.changed", task.describe());
    }

    @Override
    public void onTaskVanished(ActivityManager.RunningTaskInfo taskInfo) {
        SurfaceControl leash = leashes.remove(taskInfo.taskId);
        if (leash != null) {
            leash.release();
        }
        boolean stillRunning = TaskQueries.find(taskInfo.taskId) != null;
        Events.emit("organizer.vanished", "task=" + taskInfo.taskId + " still_running=" + stillRunning);
        if (stillRunning && reclaimPending.compareAndSet(false, true)) {
            reclaimer.schedule(this::reclaim, RECLAIM_DELAY_MS, TimeUnit.MILLISECONDS);
        }
    }

    private void takeOver() {
        long commandNanos = Events.monotonicNanos();
        List<?> existing = registerOrganizer();
        Events.emit("organizer.registered", "existing=" + existing.size() + " t_cmd=" + commandNanos);
        for (Object appeared : existing) {
            onTaskAppeared((ActivityManager.RunningTaskInfo) Hidden.call(appeared, "getTaskInfo", Hidden.NO_TYPES),
                    (SurfaceControl) Hidden.call(appeared, "getLeash", Hidden.NO_TYPES));
        }
    }

    private void reclaim() {
        Events.emit("organizer.reclaim", "");
        unregisterOrganizer();
        takeOver();
        reclaimPending.set(false);
    }

    private static void place(TaskRecord task, SurfaceControl leash, boolean firstAppearance) {
        try (SurfaceControl.Transaction transaction = new SurfaceControl.Transaction()) {
            if (task.positionInParent != null) {
                transaction.setPosition(leash, task.positionInParent.x, task.positionInParent.y);
            }
            transaction.setCrop(leash, new Rect(0, 0, task.bounds.width(), task.bounds.height()));
            if (firstAppearance) {
                transaction.setVisibility(leash, task.visible);
            }
            transaction.apply();
        }
    }
}
