package org.crownos.agent.sizing;

/** A wish to give {@code task} this size, acknowledged to the hwcomposer with {@code serial}. */
public final class SizeRequest {
    public final int task;
    public final int width;
    public final int height;
    public final int serial;

    public SizeRequest(int task, int width, int height, int serial) {
        this.task = task;
        this.width = width;
        this.height = height;
        this.serial = serial;
    }

    @Override
    public String toString() {
        return "task " + task + " " + width + "x" + height + " serial " + Integer.toUnsignedString(serial);
    }
}
