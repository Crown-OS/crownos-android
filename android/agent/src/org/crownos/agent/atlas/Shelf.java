package org.crownos.agent.atlas;

import java.util.Map;
import java.util.TreeMap;

/** One row of slots sharing a top edge, ordered left to right. */
final class Shelf {
    final int top;
    private final TreeMap<Integer, Slot> slotsByLeft = new TreeMap<>();

    Shelf(int top) {
        this.top = top;
    }

    int height() {
        int height = 0;
        for (Slot slot : slotsByLeft.values()) {
            height = Math.max(height, slot.height);
        }
        return height;
    }

    int bottom() {
        return top + height();
    }

    boolean isEmpty() {
        return slotsByLeft.isEmpty();
    }

    void add(Slot slot) {
        slotsByLeft.put(slot.left, slot);
    }

    void remove(Slot slot) {
        slotsByLeft.remove(slot.left);
    }

    /** The leftmost x where a slot this wide fits with {@code gap} to its neighbours, or -1. */
    int freeLeft(int width, int gap, int displayWidth) {
        int cursor = 0;
        for (Slot slot : slotsByLeft.values()) {
            if (cursor + width + gap <= slot.left) {
                return cursor;
            }
            cursor = slot.right() + gap;
        }
        return cursor + width <= displayWidth ? cursor : -1;
    }

    /** Whether {@code slot} can become {@code width} wide without reaching its right neighbour. */
    boolean canWiden(Slot slot, int width, int gap, int displayWidth) {
        Map.Entry<Integer, Slot> next = slotsByLeft.higherEntry(slot.left);
        int limit = next == null ? displayWidth : next.getKey() - gap;
        return slot.left + width <= limit;
    }
}
