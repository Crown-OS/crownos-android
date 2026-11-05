package org.crownos.agent;

import android.graphics.Rect;

final class Rects {
    private static final int EDGE_COUNT = 4;

    private Rects() {
    }

    static Rect parse(String[] arguments, int firstIndex) {
        if (arguments.length < firstIndex + EDGE_COUNT) {
            throw new IllegalArgumentException("expected <left> <top> <right> <bottom>");
        }
        Rect bounds = new Rect(
                Integer.parseInt(arguments[firstIndex]),
                Integer.parseInt(arguments[firstIndex + 1]),
                Integer.parseInt(arguments[firstIndex + 2]),
                Integer.parseInt(arguments[firstIndex + 3]));
        if (bounds.isEmpty()) {
            throw new IllegalArgumentException("empty bounds " + format(bounds));
        }
        return bounds;
    }

    static String format(Rect bounds) {
        return bounds == null ? "none" : bounds.left + "," + bounds.top + "," + bounds.right + "," + bounds.bottom;
    }
}
