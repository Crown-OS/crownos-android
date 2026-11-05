package org.crownos.agent.util;

/** Converts wire sizes, which are u32, into Java ints. */
public final class Dimensions {
    private Dimensions() {
    }

    /** {@code value} read as unsigned and clamped to {@code max}. */
    public static int fromWire(int value, int max) {
        return value < 0 ? max : Math.min(value, max);
    }
}
