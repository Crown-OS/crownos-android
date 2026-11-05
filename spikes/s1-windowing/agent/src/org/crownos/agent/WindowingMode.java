package org.crownos.agent;

enum WindowingMode {
    UNDEFINED(0, "undefined"),
    FULLSCREEN(1, "fullscreen"),
    PINNED(2, "pinned"),
    SPLIT_PRIMARY(3, "split-primary"),
    SPLIT_SECONDARY(4, "split-secondary"),
    FREEFORM(5, "freeform"),
    MULTI_WINDOW(6, "multiwindow");

    final int value;
    final String label;

    WindowingMode(int value, String label) {
        this.value = value;
        this.label = label;
    }

    static String labelOf(int value) {
        for (WindowingMode mode : values()) {
            if (mode.value == value) {
                return mode.label;
            }
        }
        return "mode" + value;
    }

    static WindowingMode parse(String label) {
        for (WindowingMode mode : values()) {
            if (mode.label.equals(label)) {
                return mode;
            }
        }
        throw new IllegalArgumentException("unknown windowing mode: " + label);
    }
}
