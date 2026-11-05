package org.crownos.agent.task;

import android.app.ActivityManager;
import android.view.SurfaceControl;

import java.util.HashSet;
import java.util.List;
import java.util.Set;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Loop;

/**
 * Feeds the coordinator. Normally the agent registers as the task organizer and takes the tasks
 * back within a second whenever SystemUI re-registers after a crash. In safe mode, or when this
 * image refuses the organizer, it polls the running tasks instead and never moves them.
 */
public final class TaskSource {
    private static final long RECLAIM_DELAY_MILLIS = 300;
    private static final long OWNERSHIP_CHECK_MILLIS = 450;
    private static final int UNOWNED_CHECKS_BEFORE_RECLAIM = 2;
    private static final long POLL_MILLIS = 1_000;

    private final Loop loop;
    private final TaskCoordinator coordinator;
    private TaskOrganizerBridge organizer;
    private boolean reclaimScheduled;
    private int unownedChecks;

    public TaskSource(Loop loop, TaskCoordinator coordinator) {
        this.loop = loop;
        this.coordinator = coordinator;
    }

    /** Becomes the organizer, falling back to observing. Returns the placement that took effect. */
    public TaskPlacement organize() {
        try {
            organizer = new TaskOrganizerBridge(loop, coordinator);
            coordinator.use(TaskPlacement.ORGANIZED, this::requestReclaim);
            register();
        } catch (HiddenApiException | LinkageError | IllegalStateException e) {
            AgentLog.error("cannot become the task organizer; observing tasks instead", e);
            organizer = null;
            observe();
            return TaskPlacement.OBSERVED;
        }
        loop.every(this::checkOwnership, OWNERSHIP_CHECK_MILLIS);
        return TaskPlacement.ORGANIZED;
    }

    public void observe() {
        coordinator.use(TaskPlacement.OBSERVED, () -> { });
        loop.execute(this::poll);
        loop.every(this::poll, POLL_MILLIS);
    }

    private void register() {
        List<?> existing = organizer.register();
        AgentLog.info("registered as the task organizer, took " + existing.size() + " tasks");
        for (Object appeared : existing) {
            try {
                coordinator.onAppeared(
                        (ActivityManager.RunningTaskInfo) Hidden.call(appeared, "getTaskInfo", Hidden.NO_TYPES),
                        (SurfaceControl) Hidden.call(appeared, "getLeash", Hidden.NO_TYPES));
            } catch (HiddenApiException e) {
                AgentLog.warn("cannot adopt a task: " + e.getMessage());
            }
        }
    }

    private void requestReclaim() {
        if (!reclaimScheduled) {
            reclaimScheduled = true;
            loop.schedule(this::reclaim, RECLAIM_DELAY_MILLIS);
        }
    }

    private void reclaim() {
        reclaimScheduled = false;
        unownedChecks = 0;
        AgentLog.warn("another organizer took the tasks; registering again");
        try {
            organizer.unregister();
            register();
        } catch (HiddenApiException | IllegalStateException e) {
            AgentLog.error("reclaiming the tasks failed", e);
        }
    }

    /** Catches a re-registered SystemUI even when no task vanished from the agent, for example with none open. */
    private void checkOwnership() {
        List<ActivityManager.RunningTaskInfo> running = runningTasks();
        if (running == null) {
            return;
        }
        boolean unowned = false;
        for (ActivityManager.RunningTaskInfo info : running) {
            if (Hidden.booleanField(info, "isVisible") && !coordinator.hasLeash(info.taskId)) {
                unowned = true;
                break;
            }
        }
        unownedChecks = unowned ? unownedChecks + 1 : 0;
        if (unownedChecks >= UNOWNED_CHECKS_BEFORE_RECLAIM && !reclaimScheduled) {
            reclaim();
        }
    }

    private void poll() {
        List<ActivityManager.RunningTaskInfo> tasks = runningTasks();
        if (tasks == null) {
            return;
        }
        Set<Integer> running = new HashSet<>();
        for (ActivityManager.RunningTaskInfo info : tasks) {
            running.add(info.taskId);
            if (coordinator.knows(info.taskId)) {
                coordinator.onChanged(info);
            } else {
                coordinator.onAppeared(info, null);
            }
        }
        for (Integer known : coordinator.knownTasks()) {
            if (!running.contains(known)) {
                coordinator.onRemoved(known);
            }
        }
    }

    /** The running tasks, or {@code null} after logging once that this image cannot list them. */
    private static List<ActivityManager.RunningTaskInfo> runningTasks() {
        try {
            return TaskQueries.running();
        } catch (HiddenApiException e) {
            AgentLog.warnOnce("get-tasks", "cannot list the running tasks: " + e.getMessage());
            return null;
        }
    }
}
