package org.crownos.agent.task;

import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.util.AgentLog;

/** Focus, removal, Back and z-order for single tasks. Failures are logged, never thrown. */
final class TaskControl {
    private static final int INJECT_ASYNC = 0;
    private static final int SYSTEM_KEY_FLAGS = KeyEvent.FLAG_FROM_SYSTEM | KeyEvent.FLAG_VIRTUAL_HARD_KEY;

    private TaskControl() {
    }

    static void focus(int taskId) {
        try {
            Hidden.call(Services.activityTaskManager(), "setFocusedTask", Hidden.types(int.class), taskId);
        } catch (HiddenApiException e) {
            AgentLog.warn("setFocusedTask " + taskId + " failed: " + e.getMessage());
        }
    }

    static void remove(int taskId) {
        try {
            Object removed = Hidden.call(Services.activityTaskManager(), "removeTask", Hidden.types(int.class), taskId);
            if (!Boolean.TRUE.equals(removed)) {
                AgentLog.warn("removeTask " + taskId + " found no such task");
            }
        } catch (HiddenApiException e) {
            AgentLog.warn("removeTask " + taskId + " failed: " + e.getMessage());
        }
    }

    /** Injects a Back key press into the focused window of {@code displayId}. */
    static void back(int displayId) {
        long now = SystemClock.uptimeMillis();
        KeyEvent down = new KeyEvent(now, now, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK, 0, 0,
                KeyCharacterMap.VIRTUAL_KEYBOARD, 0, SYSTEM_KEY_FLAGS, InputDevice.SOURCE_KEYBOARD);
        try {
            inject(down, displayId);
            inject(KeyEvent.changeAction(down, KeyEvent.ACTION_UP), displayId);
        } catch (HiddenApiException e) {
            AgentLog.warn("injecting Back into display " + displayId + " failed: " + e.getMessage());
        }
    }

    /** Puts the task above every other task, for slots that overlap after an overflow. */
    static void raise(Object token) {
        try {
            WindowTransactions.apply(WindowTransactions.raise(token));
        } catch (HiddenApiException e) {
            AgentLog.warn("raising a task failed: " + e.getMessage());
        }
    }

    private static void inject(KeyEvent event, int displayId) {
        Hidden.call(event, "setDisplayId", Hidden.types(int.class), displayId);
        Hidden.call(Services.inputManager(), "injectInputEvent", Hidden.types(InputEvent.class, int.class), event,
                INJECT_ASYNC);
    }
}
