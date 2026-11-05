package org.crownos.agent.task;

import android.os.SystemClock;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.crownos.agent.parking.BackParking;

/** Every task the agent knows, by id, and which of them Back parked. Touched only on the agent loop. */
final class TaskTable {
    private final Map<Integer, TaskState> tasks = new HashMap<>();
    private final BackParking parking = new BackParking(SystemClock::uptimeMillis);

    TaskState get(int taskId) {
        return tasks.get(taskId);
    }

    TaskState getOrAdd(int taskId) {
        TaskState task = tasks.get(taskId);
        if (task == null) {
            task = new TaskState(taskId);
            tasks.put(taskId, task);
        }
        return task;
    }

    void remove(int taskId) {
        tasks.remove(taskId);
        parking.forget(taskId);
    }

    /** Which tasks have no window because Back sent them behind the others. */
    BackParking parking() {
        return parking;
    }

    boolean isParked(int taskId) {
        return parking.isParked(taskId);
    }

    /** The task when it is a window, otherwise {@code null}. */
    TaskState window(int taskId) {
        TaskState task = tasks.get(taskId);
        return task != null && task.isWindow() ? task : null;
    }

    boolean contains(int taskId) {
        return tasks.containsKey(taskId);
    }

    boolean hasLeash(int taskId) {
        TaskState task = tasks.get(taskId);
        return task != null && task.leash != null;
    }

    List<Integer> ids() {
        return new ArrayList<>(tasks.keySet());
    }

    List<TaskState> windows() {
        List<TaskState> windows = new ArrayList<>();
        for (TaskState task : tasks.values()) {
            if (task.isWindow()) {
                windows.add(task);
            }
        }
        return windows;
    }

    int[] announcedIds() {
        int[] ids = new int[tasks.size()];
        int count = 0;
        for (TaskState task : tasks.values()) {
            if (task.announced) {
                ids[count++] = task.id;
            }
        }
        return Arrays.copyOf(ids, count);
    }
}
