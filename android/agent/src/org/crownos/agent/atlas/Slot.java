package org.crownos.agent.atlas;

/** A task's rectangle on the Android display, in pixels. */
public final class Slot {
    public final int left;
    public final int top;
    public final int width;
    public final int height;

    public Slot(int left, int top, int width, int height) {
        this.left = left;
        this.top = top;
        this.width = width;
        this.height = height;
    }

    public int right() {
        return left + width;
    }

    public int bottom() {
        return top + height;
    }

    public boolean sameSize(int otherWidth, int otherHeight) {
        return width == otherWidth && height == otherHeight;
    }

    /** Whether this and {@code other} are at least {@code gap} pixels apart on some axis. */
    public boolean separatedFrom(Slot other, int gap) {
        return right() + gap <= other.left || other.right() + gap <= left
                || bottom() + gap <= other.top || other.bottom() + gap <= top;
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof Slot)) {
            return false;
        }
        Slot slot = (Slot) other;
        return left == slot.left && top == slot.top && width == slot.width && height == slot.height;
    }

    @Override
    public int hashCode() {
        return ((left * 31 + top) * 31 + width) * 31 + height;
    }

    @Override
    public String toString() {
        return width + "x" + height + "+" + left + "+" + top;
    }
}
