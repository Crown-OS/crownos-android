package org.crownos.agent.task;

import android.graphics.Rect;

import org.crownos.agent.atlas.Slot;
import org.crownos.agent.util.Dimensions;

/** The fixed Android display the atlas covers, and task rectangles on it. */
final class DisplayGeometry {
    final int width;
    final int height;

    DisplayGeometry(int width, int height) {
        this.width = width;
        this.height = height;
    }

    int wireWidth(int value) {
        return Dimensions.fromWire(value, width);
    }

    int wireHeight(int value) {
        return Dimensions.fromWire(value, height);
    }

    /** Where the task is now, by Android's account. */
    Slot boundsOf(TaskSnapshot snapshot) {
        Rect bounds = snapshot.bounds;
        return new Slot(snapshot.positionInParent.x, snapshot.positionInParent.y,
                bounds.isEmpty() ? width : bounds.width(), bounds.isEmpty() ? height : bounds.height());
    }

    /** Whether a task chose its own freeform size, rather than filling the display. */
    boolean hasOwnSize(TaskSnapshot snapshot) {
        Rect bounds = snapshot.bounds;
        return snapshot.isFreeform() && !bounds.isEmpty() && (bounds.width() < width || bounds.height() < height);
    }
}
