package org.crownos.agent;

import android.os.Bundle;

final class TaskControl {
    private TaskControl() {
    }

    static void focus(int taskId) {
        long commandNanos = Events.monotonicNanos();
        Hidden.call(Services.activityTaskManager(), "setFocusedTask", new Class<?>[] {int.class}, taskId);
        Events.emit("focus", "task=" + taskId + " t_cmd=" + commandNanos);
    }

    static void moveToFront(int taskId) {
        long commandNanos = Events.monotonicNanos();
        Hidden.call(Services.activityTaskManager(), "moveTaskToFront",
                new Class<?>[] {Hidden.type("android.app.IApplicationThread"), String.class, int.class, int.class,
                    Bundle.class},
                null, Services.CALLER_PACKAGE, taskId, 0, null);
        Events.emit("front", "task=" + taskId + " t_cmd=" + commandNanos);
    }

    static void remove(int taskId) {
        long commandNanos = Events.monotonicNanos();
        boolean removed = (Boolean) Hidden.call(Services.activityTaskManager(), "removeTask",
                new Class<?>[] {int.class}, taskId);
        Events.emit("remove", "task=" + taskId + " result=" + removed + " t_cmd=" + commandNanos);
    }
}
