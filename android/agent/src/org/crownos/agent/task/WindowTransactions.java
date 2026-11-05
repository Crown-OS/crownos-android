package org.crownos.agent.task;

import android.graphics.Rect;
import android.window.WindowContainerTransactionCallback;

import org.crownos.agent.platform.Hidden;

/** WindowContainerTransactions through the hidden {@code WindowOrganizer}. */
final class WindowTransactions {
    private static final String TRANSACTION = "android.window.WindowContainerTransaction";
    private static final String TOKEN = "android.window.WindowContainerToken";

    private static Object organizer;

    private WindowTransactions() {
    }

    /** A transaction that gives the task {@code bounds}, switching it to FREEFORM first when asked. */
    static Object bounds(Object token, Rect bounds, WindowingFix windowingFix) {
        Object transaction = Hidden.construct(Hidden.type(TRANSACTION), Hidden.NO_TYPES);
        if (windowingFix == WindowingFix.MAKE_FREEFORM) {
            Hidden.call(transaction, "setWindowingMode", Hidden.types(TOKEN, int.class), token,
                    TaskSnapshot.WINDOWING_MODE_FREEFORM);
        }
        Hidden.call(transaction, "setBounds", Hidden.types(TOKEN, Rect.class), token, bounds);
        return transaction;
    }

    static Object raise(Object token) {
        Object transaction = Hidden.construct(Hidden.type(TRANSACTION), Hidden.NO_TYPES);
        Hidden.call(transaction, "reorder", Hidden.types(TOKEN, boolean.class), token, true);
        return transaction;
    }

    static void apply(Object transaction) {
        Hidden.call(organizer(), "applyTransaction", Hidden.types(TRANSACTION), transaction);
    }

    /** Applies {@code transaction} once every window in it has redrawn; {@code callback} gets the result. */
    static void applySync(Object transaction, WindowContainerTransactionCallback callback) {
        Hidden.call(organizer(), "applySyncTransaction",
                Hidden.types(TRANSACTION, WindowContainerTransactionCallback.class), transaction, callback);
    }

    private static synchronized Object organizer() {
        if (organizer == null) {
            organizer = Hidden.construct(Hidden.type("android.window.WindowOrganizer"), Hidden.NO_TYPES);
        }
        return organizer;
    }

    /** Whether a transaction also restores FREEFORM, for tasks that left it (F11, patch 0015). */
    enum WindowingFix {
        KEEP_MODE,
        MAKE_FREEFORM
    }
}
