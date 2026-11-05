package org.crownos.agent.task;

import android.graphics.Rect;
import android.view.SurfaceControl;

import org.crownos.agent.atlas.Slot;

/** Positions and crops task leashes, the surfaces the organizer owns. */
final class Leashes {
    private Leashes() {
    }

    /** Places a leash that just appeared and makes it visible; organized tasks start hidden. */
    static void show(SurfaceControl leash, Slot slot) {
        try (SurfaceControl.Transaction transaction = new SurfaceControl.Transaction()) {
            place(transaction, leash, slot).setVisibility(leash, true).apply();
        }
    }

    static void move(SurfaceControl leash, Slot slot) {
        try (SurfaceControl.Transaction transaction = new SurfaceControl.Transaction()) {
            place(transaction, leash, slot).apply();
        }
    }

    /** Hides the leash of a task whose window closed while the task keeps running. */
    static void hide(SurfaceControl leash) {
        try (SurfaceControl.Transaction transaction = new SurfaceControl.Transaction()) {
            transaction.setVisibility(leash, false).apply();
        }
    }

    static SurfaceControl.Transaction place(SurfaceControl.Transaction transaction, SurfaceControl leash, Slot slot) {
        return transaction.setPosition(leash, slot.left, slot.top)
                .setCrop(leash, new Rect(0, 0, slot.width, slot.height));
    }

    static void release(SurfaceControl leash) {
        if (leash != null) {
            leash.release();
        }
    }
}
