package org.crownos.agent.parking;

import org.crownos.agent.testing.Check;
import org.crownos.agent.testing.TestRunner;

public final class BackParkingTest {
    private static final int TASK = 7;
    private static final int OTHER = 9;

    private BackParkingTest() {
    }

    /** A clock the test moves by hand. */
    private static final class FakeClock {
        long millis = 50_000;

        void advance(long by) {
            millis += by;
        }
    }

    public static void run(TestRunner runner) {
        runner.test("parking: Back followed by leaving the front within the window parks the task", () -> {
            FakeClock clock = new FakeClock();
            BackParking parking = new BackParking(() -> clock.millis);
            parking.backPressed(TASK);
            clock.advance(BackParking.BACK_WINDOW_MILLIS);
            Check.isTrue(parking.leftFront(TASK), "parked at the edge of the window");
            Check.isTrue(parking.isParked(TASK), "marked parked");
        });

        runner.test("parking: leaving the front too late or without Back keeps the window", () -> {
            FakeClock clock = new FakeClock();
            BackParking parking = new BackParking(() -> clock.millis);
            Check.isTrue(!parking.leftFront(TASK), "no Back at all");
            parking.backPressed(TASK);
            clock.advance(BackParking.BACK_WINDOW_MILLIS + 1);
            Check.isTrue(!parking.leftFront(TASK), "Back too long ago");
            parking.backPressed(OTHER);
            Check.isTrue(!parking.leftFront(TASK), "Back went to another task");
            Check.isTrue(!parking.isParked(TASK), "never parked");
        });

        runner.test("parking: one Back parks at most once", () -> {
            FakeClock clock = new FakeClock();
            BackParking parking = new BackParking(() -> clock.millis);
            parking.backPressed(TASK);
            Check.isTrue(parking.leftFront(TASK), "first departure parks");
            Check.isTrue(parking.cameToFront(TASK), "re-front unparks");
            Check.isTrue(!parking.leftFront(TASK), "a later departure needs a new Back");
        });

        runner.test("parking: focusing another window explains the focus loss", () -> {
            FakeClock clock = new FakeClock();
            BackParking parking = new BackParking(() -> clock.millis);
            parking.backPressed(TASK);
            parking.hostFocused(TASK);
            clock.advance(10);
            Check.isTrue(parking.leftFront(TASK), "focusing the same window keeps the Back");
            parking.cameToFront(TASK);
            parking.backPressed(TASK);
            parking.hostFocused(OTHER);
            Check.isTrue(!parking.leftFront(TASK), "the user switched windows right after Back");
        });

        runner.test("parking: re-front unparks only parked tasks, once", () -> {
            FakeClock clock = new FakeClock();
            BackParking parking = new BackParking(() -> clock.millis);
            Check.isTrue(!parking.cameToFront(TASK), "never parked");
            parking.backPressed(TASK);
            parking.leftFront(TASK);
            Check.isTrue(parking.cameToFront(TASK), "unparked");
            Check.isTrue(!parking.isParked(TASK), "no longer parked");
            Check.isTrue(!parking.cameToFront(TASK), "second front is an ordinary one");
        });

        runner.test("parking: a removed task forgets its Back and its parking", () -> {
            FakeClock clock = new FakeClock();
            BackParking parking = new BackParking(() -> clock.millis);
            parking.backPressed(TASK);
            parking.leftFront(TASK);
            parking.forget(TASK);
            Check.isTrue(!parking.isParked(TASK), "parking forgotten");
            parking.backPressed(OTHER);
            parking.forget(OTHER);
            Check.isTrue(!parking.leftFront(OTHER), "Back forgotten");
        });
    }
}
