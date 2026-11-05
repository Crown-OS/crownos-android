package org.crownos.agent;

import android.app.ActivityManager;
import android.content.ComponentName;
import android.content.res.Configuration;
import android.graphics.Point;
import android.graphics.Rect;

import java.util.List;
import java.util.Locale;

final class TaskRecord {
    final int taskId;
    final int windowingMode;
    final Rect bounds;
    final Point positionInParent;
    final boolean visible;
    final boolean focused;
    final boolean resizeable;
    final int resizeMode;
    final boolean supportsMultiWindow;
    final int minWidth;
    final int minHeight;
    final int defaultMinSize;
    final int displayId;
    final ComponentName baseActivity;
    final ComponentName topActivity;
    final ActivityManager.TaskDescription description;
    final Object token;
    final List<?> launchCookies;

    private TaskRecord(ActivityManager.RunningTaskInfo info) {
        Object windowConfiguration = Hidden.field((Configuration) Hidden.field(info, "configuration"),
                "windowConfiguration");
        taskId = info.taskId;
        windowingMode = (Integer) Hidden.call(windowConfiguration, "getWindowingMode", Hidden.NO_TYPES);
        bounds = (Rect) Hidden.call(windowConfiguration, "getBounds", Hidden.NO_TYPES);
        positionInParent = (Point) Hidden.field(info, "positionInParent");
        visible = Hidden.booleanField(info, "isVisible");
        focused = Hidden.booleanField(info, "isFocused");
        resizeable = Hidden.booleanField(info, "isResizeable");
        resizeMode = Hidden.intField(info, "resizeMode");
        supportsMultiWindow = Hidden.booleanField(info, "supportsMultiWindow");
        minWidth = Hidden.intField(info, "minWidth");
        minHeight = Hidden.intField(info, "minHeight");
        defaultMinSize = Hidden.intField(info, "defaultMinSize");
        displayId = Hidden.intField(info, "displayId");
        baseActivity = info.baseActivity;
        topActivity = info.topActivity;
        description = info.taskDescription;
        token = Hidden.field(info, "token");
        launchCookies = (List<?>) Hidden.field(info, "launchCookies");
    }

    static TaskRecord of(ActivityManager.RunningTaskInfo info) {
        return new TaskRecord(info);
    }

    String packageName() {
        ComponentName component = baseActivity != null ? baseActivity : topActivity;
        return component == null ? null : component.getPackageName();
    }

    String describe() {
        return String.format(Locale.ROOT,
                "task=%d mode=%s bounds=%s pos=%s visible=%b focused=%b resizeable=%b resizeMode=%d"
                        + " multiwindow=%b min=%dx%d defaultMin=%d display=%d base=%s top=%s label=\"%s\""
                        + " color=#%08x background=#%08x statusbar=#%08x",
                taskId, WindowingMode.labelOf(windowingMode), Rects.format(bounds),
                positionInParent == null ? "none" : positionInParent.x + "," + positionInParent.y,
                visible, focused, resizeable, resizeMode, supportsMultiWindow, minWidth, minHeight, defaultMinSize,
                displayId, flatten(baseActivity), flatten(topActivity),
                description == null ? "" : String.valueOf(description.getLabel()),
                description == null ? 0 : description.getPrimaryColor(),
                description == null ? 0 : description.getBackgroundColor(),
                description == null ? 0 : description.getStatusBarColor());
    }

    private static String flatten(ComponentName component) {
        return component == null ? "none" : component.flattenToShortString();
    }
}
