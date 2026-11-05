package org.crownos.agent.atlas;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Packs task slots into shelves on the fixed Android display, keeping every pair of slots at least
 * {@link #GAP} pixels apart. Placing a task never moves another one. When nothing fits, the task
 * overlaps the slot of a hidden or least recently focused task instead.
 */
public final class Atlas {
    /** Minimum distance between two slots, so no shadow, inset or input slop reaches a neighbour. */
    public static final int GAP = 64;

    private final int displayWidth;
    private final int displayHeight;
    private final List<Shelf> shelves = new ArrayList<>();
    private final Map<Integer, Slot> slots = new HashMap<>();
    private final Map<Integer, Shelf> shelfOf = new HashMap<>();
    private final List<Integer> focusOrder = new ArrayList<>();
    private final Set<Integer> hidden = new HashSet<>();

    public Atlas(int displayWidth, int displayHeight) {
        this.displayWidth = displayWidth;
        this.displayHeight = displayHeight;
    }

    /**
     * Gives {@code key} a slot of this size, clamped to the display. The slot keeps its origin when
     * the new size still fits there.
     */
    public Slot place(int key, int width, int height) {
        int clampedWidth = clamp(width, displayWidth);
        int clampedHeight = clamp(height, displayHeight);
        Slot current = slots.get(key);
        Shelf shelf = shelfOf.get(key);
        if (current != null && shelf != null && fitsInPlace(shelf, current, clampedWidth, clampedHeight)) {
            Slot resized = new Slot(current.left, current.top, clampedWidth, clampedHeight);
            shelf.remove(current);
            shelf.add(resized);
            slots.put(key, resized);
            return resized;
        }
        release(key);
        Slot free = insertFree(key, clampedWidth, clampedHeight);
        if (free != null) {
            return free;
        }
        Slot overlapping = overlapping(clampedWidth, clampedHeight);
        slots.put(key, overlapping);
        return overlapping;
    }

    /** The slot of {@code key}, or {@code null}. */
    public Slot slotOf(int key) {
        return slots.get(key);
    }

    /** Whether {@code key} overlaps another slot because the display was full. */
    public boolean isOverflowed(int key) {
        return slots.containsKey(key) && !shelfOf.containsKey(key);
    }

    /**
     * Forgets {@code key}. Returns the overflowed keys that moved into the freed space, most recently
     * focused first; their new slots are in {@link #slotOf}.
     */
    public List<Integer> remove(int key) {
        release(key);
        focusOrder.remove(Integer.valueOf(key));
        hidden.remove(key);
        List<Integer> relieved = new ArrayList<>();
        for (Integer candidate : stackingOrder()) {
            if (!isOverflowed(candidate)) {
                continue;
            }
            Slot overlapping = slots.remove(candidate);
            if (insertFree(candidate, overlapping.width, overlapping.height) != null) {
                relieved.add(candidate);
            } else {
                slots.put(candidate, overlapping);
            }
        }
        return relieved;
    }

    /** Hands the slot and focus history of {@code from} to {@code to}, such as a reservation to its task. */
    public void rename(int from, int to) {
        Slot slot = slots.remove(from);
        if (slot == null) {
            return;
        }
        slots.put(to, slot);
        Shelf shelf = shelfOf.remove(from);
        if (shelf != null) {
            shelfOf.put(to, shelf);
        }
        int focusIndex = focusOrder.indexOf(from);
        if (focusIndex >= 0) {
            focusOrder.set(focusIndex, to);
        }
        if (hidden.remove(from)) {
            hidden.add(to);
        }
    }

    public void focus(int key) {
        focusOrder.remove(Integer.valueOf(key));
        focusOrder.add(0, key);
    }

    public void setVisible(int key, boolean visible) {
        if (visible) {
            hidden.remove(key);
        } else {
            hidden.add(key);
        }
    }

    /** Placed keys from top to bottom: most recently focused first, then never-focused keys, newest first. */
    public List<Integer> stackingOrder() {
        List<Integer> order = new ArrayList<>(slots.size());
        for (Integer key : focusOrder) {
            if (slots.containsKey(key)) {
                order.add(key);
            }
        }
        List<Integer> unfocused = new ArrayList<>();
        for (Integer key : slots.keySet()) {
            if (!focusOrder.contains(key)) {
                unfocused.add(key);
            }
        }
        Collections.sort(unfocused, Collections.reverseOrder());
        order.addAll(unfocused);
        return order;
    }

    private boolean fitsInPlace(Shelf shelf, Slot current, int width, int height) {
        return shelf.top + height <= ceiling(shelves.indexOf(shelf))
                && shelf.canWiden(current, width, GAP, displayWidth);
    }

    private Slot insertFree(int key, int width, int height) {
        Shelf tightest = null;
        int tightestLeft = -1;
        int tightestLeftover = Integer.MAX_VALUE;
        for (Shelf shelf : shelves) {
            int leftover = shelf.height() - height;
            if (leftover < 0 || leftover >= tightestLeftover) {
                continue;
            }
            int left = shelf.freeLeft(width, GAP, displayWidth);
            if (left >= 0) {
                tightest = shelf;
                tightestLeft = left;
                tightestLeftover = leftover;
            }
        }
        if (tightest != null) {
            return store(key, tightest, tightestLeft, width, height);
        }
        Shelf opened = openShelf(height);
        if (opened != null) {
            return store(key, opened, 0, width, height);
        }
        for (int index = 0; index < shelves.size(); index++) {
            Shelf shelf = shelves.get(index);
            int left = shelf.freeLeft(width, GAP, displayWidth);
            if (left >= 0 && shelf.top + height <= ceiling(index)) {
                return store(key, shelf, left, width, height);
            }
        }
        return null;
    }

    private Shelf openShelf(int height) {
        int top = 0;
        for (int index = 0; index <= shelves.size(); index++) {
            int limit = index < shelves.size() ? shelves.get(index).top - GAP : displayHeight;
            if (top + height <= limit) {
                Shelf shelf = new Shelf(top);
                shelves.add(index, shelf);
                return shelf;
            }
            if (index < shelves.size()) {
                top = shelves.get(index).bottom() + GAP;
            }
        }
        return null;
    }

    private Slot overlapping(int width, int height) {
        List<Integer> leastRecentFirst = stackingOrder();
        Collections.reverse(leastRecentFirst);
        Integer victim = leastRecentFirst.isEmpty() ? null : leastRecentFirst.get(0);
        for (Integer candidate : leastRecentFirst) {
            if (hidden.contains(candidate)) {
                victim = candidate;
                break;
            }
        }
        Slot base = victim == null ? null : slots.get(victim);
        int left = base == null ? 0 : Math.min(base.left, displayWidth - width);
        int top = base == null ? 0 : Math.min(base.top, displayHeight - height);
        return new Slot(left, top, width, height);
    }

    private int ceiling(int shelfIndex) {
        return shelfIndex + 1 < shelves.size() ? shelves.get(shelfIndex + 1).top - GAP : displayHeight;
    }

    private Slot store(int key, Shelf shelf, int left, int width, int height) {
        Slot slot = new Slot(left, shelf.top, width, height);
        shelf.add(slot);
        slots.put(key, slot);
        shelfOf.put(key, shelf);
        return slot;
    }

    private void release(int key) {
        Slot slot = slots.remove(key);
        Shelf shelf = shelfOf.remove(key);
        if (shelf == null) {
            return;
        }
        shelf.remove(slot);
        if (shelf.isEmpty()) {
            shelves.remove(shelf);
        }
    }

    private static int clamp(int value, int max) {
        return Math.max(1, Math.min(value, max));
    }
}
