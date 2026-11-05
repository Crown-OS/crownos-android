package org.crownos.agent.testing;

import java.util.Objects;

/** Assertions for the host test runner. */
public final class Check {
    /** A test body that may throw. */
    public interface Body {
        void run() throws Exception;
    }

    private Check() {
    }

    public static void equal(Object expected, Object actual, String what) {
        if (!Objects.equals(expected, actual)) {
            throw new AssertionError(what + ": expected <" + expected + "> but was <" + actual + ">");
        }
    }

    public static void isTrue(boolean condition, String what) {
        if (!condition) {
            throw new AssertionError(what);
        }
    }

    public static <E extends Throwable> E raises(Class<E> expected, Body body, String what) {
        try {
            body.run();
        } catch (Throwable thrown) {
            if (expected.isInstance(thrown)) {
                return expected.cast(thrown);
            }
            throw new AssertionError(what + ": expected " + expected.getSimpleName() + " but got " + thrown, thrown);
        }
        throw new AssertionError(what + ": expected " + expected.getSimpleName() + " but nothing was thrown");
    }
}
