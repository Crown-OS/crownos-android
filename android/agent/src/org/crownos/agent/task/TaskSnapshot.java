package org.crownos.agent.task;

import android.app.ActivityManager;
import android.content.ComponentName;
import android.content.res.Configuration;
import android.graphics.Point;
import android.graphics.Rect;
import android.os.IBinder;
import android.util.DisplayMetrics;

import java.util.List;

import org.crownos.agent.platform.Hidden;

/** The facts the agent uses from one {@code RunningTaskInfo}, read once through reflection. */
final class TaskSnapshot {
    static final int WINDOWING_MODE_FREEFORM = 5;
    private static final int ACTIVITY_TYPE_STANDARD = 1;
    private static final int UNSET = -1;
    private static final int OPAQUE = 0xFF000000;

    final int id;
    final int displayId;
    final int activityType;
    final int windowingMode;
    final Rect bounds;
    final Point positionInParent;
    final boolean visible;
    final boolean focused;
    final boolean resizable;
    final int minWidth;
    final int minHeight;
    final String packageName;
    final String descriptionLabel;
    final int color;
    final Object token;
    private final List<?> launchCookies;

    private TaskSnapshot(ActivityManager.RunningTaskInfo info) {
        Configuration configuration = (Configuration) Hidden.field(info, "configuration");
        Object windowConfiguration = Hidden.field(configuration, "windowConfiguration");
        id = info.taskId;
        displayId = Hidden.intField(info, "displayId");
        activityType = (Integer) Hidden.call(windowConfiguration, "getActivityType", Hidden.NO_TYPES);
        windowingMode = (Integer) Hidden.call(windowConfiguration, "getWindowingMode", Hidden.NO_TYPES);
        bounds = new Rect((Rect) Hidden.call(windowConfiguration, "getBounds", Hidden.NO_TYPES));
        Point position = (Point) Hidden.field(info, "positionInParent");
        positionInParent = position == null ? new Point(bounds.left, bounds.top) : new Point(position);
        visible = Hidden.booleanField(info, "isVisible");
        focused = Hidden.booleanField(info, "isFocused");
        resizable = Hidden.booleanField(info, "isResizeable");
        int defaultMinPixels = Hidden.intField(info, "defaultMinSize") * configuration.densityDpi
                / DisplayMetrics.DENSITY_DEFAULT;
        minWidth = orDefault(Hidden.intField(info, "minWidth"), defaultMinPixels);
        minHeight = orDefault(Hidden.intField(info, "minHeight"), defaultMinPixels);
        packageName = packageOf(info);
        ActivityManager.TaskDescription description = info.taskDescription;
        CharSequence label = description == null ? null : description.getLabel();
        descriptionLabel = label == null ? null : label.toString();
        int primary = description == null ? 0 : description.getPrimaryColor();
        color = (primary & OPAQUE) == 0 ? 0 : primary;
        token = Hidden.field(info, "token");
        launchCookies = (List<?>) Hidden.field(info, "launchCookies");
    }

    static TaskSnapshot of(ActivityManager.RunningTaskInfo info) {
        return new TaskSnapshot(info);
    }

    /** An ordinary app task, which becomes a host window. Home and system tasks do not. */
    boolean isApp() {
        return activityType == ACTIVITY_TYPE_STANDARD && packageName != null;
    }

    boolean isFreeform() {
        return windowingMode == WINDOWING_MODE_FREEFORM;
    }

    boolean hasLaunchCookie(IBinder cookie) {
        return launchCookies != null && launchCookies.contains(cookie);
    }

    private static int orDefault(int pixels, int fallback) {
        return pixels == UNSET ? fallback : pixels;
    }

    private static String packageOf(ActivityManager.RunningTaskInfo info) {
        ComponentName component = info.baseActivity != null ? info.baseActivity : info.topActivity;
        if (component == null && info.baseIntent != null) {
            component = info.baseIntent.getComponent();
        }
        return component == null ? null : component.getPackageName();
    }
}
