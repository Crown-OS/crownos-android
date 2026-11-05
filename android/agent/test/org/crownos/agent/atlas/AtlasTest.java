package org.crownos.agent.atlas;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Random;

import org.crownos.agent.testing.Check;
import org.crownos.agent.testing.TestRunner;

public final class AtlasTest {
    private static final int DISPLAY = 4096;
    private static final int QUARTER = 2000;
    private static final int RANDOM_ROUNDS = 4000;
    private static final long SEED = 20261105L;

    private AtlasTest() {
    }

    private static void assertPacked(Atlas atlas, List<Integer> keys) {
        for (int first = 0; first < keys.size(); first++) {
            Slot a = atlas.slotOf(keys.get(first));
            if (atlas.isOverflowed(keys.get(first))) {
                continue;
            }
            Check.isTrue(a.left >= 0 && a.top >= 0 && a.right() <= DISPLAY && a.bottom() <= DISPLAY,
                    "slot " + a + " inside the display");
            for (int second = first + 1; second < keys.size(); second++) {
                Slot b = atlas.slotOf(keys.get(second));
                if (!atlas.isOverflowed(keys.get(second))) {
                    Check.isTrue(a.separatedFrom(b, Atlas.GAP), a + " and " + b + " keep the gap");
                }
            }
        }
    }

    private static Atlas fullAtlas() {
        Atlas atlas = new Atlas(DISPLAY, DISPLAY);
        for (int task = 1; task <= 4; task++) {
            atlas.place(task, QUARTER, QUARTER);
            atlas.focus(task);
        }
        return atlas;
    }

    public static void run(TestRunner runner) {
        runner.test("atlas: slots in a row and the next shelf keep exactly the gap", () -> {
            Atlas atlas = new Atlas(DISPLAY, DISPLAY);
            Check.equal(new Slot(0, 0, 1000, 1000), atlas.place(1, 1000, 1000), "first");
            Check.equal(new Slot(1064, 0, 1000, 1000), atlas.place(2, 1000, 1000), "second");
            Check.equal(new Slot(2128, 0, 1000, 1000), atlas.place(3, 1000, 1000), "third");
            Check.equal(new Slot(0, 1064, 1000, 1000), atlas.place(4, 1000, 1000), "row full, new shelf");
            Check.equal(new Slot(3192, 0, 900, 500), atlas.place(5, 900, 500), "a short slot fits the first row");
        });

        runner.test("atlas: random places and removals never overlap and always keep the gap", () -> {
            Random random = new Random(SEED);
            Atlas atlas = new Atlas(DISPLAY, DISPLAY);
            List<Integer> keys = new ArrayList<>();
            for (int round = 0; round < RANDOM_ROUNDS; round++) {
                int key = 1 + random.nextInt(14);
                if (keys.contains(key) && random.nextInt(3) == 0) {
                    atlas.remove(key);
                    keys.remove(Integer.valueOf(key));
                } else {
                    atlas.place(key, 200 + random.nextInt(1800), 200 + random.nextInt(1400));
                    if (!keys.contains(key)) {
                        keys.add(key);
                    }
                    atlas.focus(key);
                }
                assertPacked(atlas, keys);
            }
        });

        runner.test("atlas: a resize that still fits keeps the origin and moves nobody else", () -> {
            Atlas atlas = new Atlas(DISPLAY, DISPLAY);
            atlas.place(1, 1000, 1000);
            Slot neighbour = atlas.place(2, 1000, 1000);
            Check.equal(new Slot(0, 0, 1000, 1300), atlas.place(1, 1000, 1300), "taller in place");
            Slot moved = atlas.place(1, 1001, 1300);
            Check.isTrue(moved.left != 0 || moved.top != 0, "too wide for its place, so it moved: " + moved);
            Check.equal(neighbour, atlas.slotOf(2), "neighbour untouched");
            assertPacked(atlas, Arrays.asList(1, 2));
        });

        runner.test("atlas: sizes are clamped to the display", () -> {
            Atlas atlas = new Atlas(DISPLAY, DISPLAY);
            Check.equal(new Slot(0, 0, DISPLAY, 1), atlas.place(1, 10000, 0), "clamped");
        });

        runner.test("atlas: overflow overlaps the least recently focused task and stacks the newest on top", () -> {
            Atlas atlas = fullAtlas();
            Slot overflow = atlas.place(5, QUARTER, QUARTER);
            Check.isTrue(atlas.isOverflowed(5), "fifth task overflows");
            Slot leastRecent = atlas.slotOf(1);
            Check.equal(new Slot(leastRecent.left, leastRecent.top, QUARTER, QUARTER), overflow,
                    "overlaps task 1");
            atlas.focus(5);
            Check.equal(Arrays.asList(5, 4, 3, 2, 1), atlas.stackingOrder(), "most recently focused on top");
            atlas.focus(1);
            Check.equal(Integer.valueOf(1), atlas.stackingOrder().get(0), "refocus raises task 1");
        });

        runner.test("atlas: overflow prefers a hidden task over a visible one", () -> {
            Atlas atlas = fullAtlas();
            atlas.setVisible(3, false);
            Slot overflow = atlas.place(5, QUARTER, QUARTER);
            Slot hidden = atlas.slotOf(3);
            Check.equal(hidden.left, overflow.left, "left of the hidden task");
            Check.equal(hidden.top, overflow.top, "top of the hidden task");
        });

        runner.test("atlas: overflowed tasks move into freed space when a task goes away", () -> {
            Atlas atlas = fullAtlas();
            atlas.place(5, QUARTER, QUARTER);
            atlas.focus(5);
            Check.equal(Arrays.asList(5), atlas.remove(2), "relieved");
            Check.isTrue(!atlas.isOverflowed(5), "no longer overflowed");
            assertPacked(atlas, Arrays.asList(1, 3, 4, 5));
            Check.equal(Arrays.asList(), atlas.remove(5), "nothing left to relieve");
        });

        runner.test("atlas: a reservation hands its slot to the task that appears", () -> {
            Atlas atlas = new Atlas(DISPLAY, DISPLAY);
            Slot reserved = atlas.place(-1, 800, 600);
            atlas.rename(-1, 42);
            Check.equal(reserved, atlas.slotOf(42), "slot moved to the task");
            Check.equal(null, atlas.slotOf(-1), "reservation gone");
            Check.equal(new Slot(800 + Atlas.GAP, 0, 800, 600), atlas.place(43, 800, 600), "space stays taken");
        });

        runner.test("atlas: freed shelves are reused from the top", () -> {
            Atlas atlas = new Atlas(DISPLAY, DISPLAY);
            atlas.place(1, DISPLAY, 1000);
            atlas.place(2, DISPLAY, 1000);
            atlas.remove(1);
            Check.equal(new Slot(0, 0, DISPLAY, 900), atlas.place(3, DISPLAY, 900), "first shelf reused");
        });
    }
}
