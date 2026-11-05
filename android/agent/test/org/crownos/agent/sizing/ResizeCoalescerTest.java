package org.crownos.agent.sizing;

import org.crownos.agent.testing.Check;
import org.crownos.agent.testing.TestRunner;

public final class ResizeCoalescerTest {
    private ResizeCoalescerTest() {
    }

    public static void run(TestRunner runner) {
        runner.test("sizing: one request in flight, the latest one pending", () -> {
            ResizeCoalescer coalescer = new ResizeCoalescer();
            SizeRequest first = new SizeRequest(7, 100, 100, 1);
            Check.equal(first, coalescer.submit(first), "first starts at once");
            Check.equal(null, coalescer.submit(new SizeRequest(7, 200, 200, 2)), "second waits");
            SizeRequest third = new SizeRequest(7, 300, 300, 3);
            Check.equal(null, coalescer.submit(third), "third replaces the second");
            Check.equal(third, coalescer.complete(first), "only the latest runs next");
            Check.equal(null, coalescer.complete(third), "nothing pending");
            Check.isTrue(!coalescer.isBusy(7), "idle");
        });

        runner.test("sizing: tasks do not wait for each other", () -> {
            ResizeCoalescer coalescer = new ResizeCoalescer();
            SizeRequest a = new SizeRequest(1, 100, 100, 1);
            SizeRequest b = new SizeRequest(2, 100, 100, 1);
            Check.equal(a, coalescer.submit(a), "task 1");
            Check.equal(b, coalescer.submit(b), "task 2");
        });

        runner.test("sizing: a stale completion changes nothing", () -> {
            ResizeCoalescer coalescer = new ResizeCoalescer();
            SizeRequest first = new SizeRequest(7, 100, 100, 1);
            coalescer.submit(first);
            SizeRequest second = new SizeRequest(7, 200, 200, 2);
            coalescer.submit(second);
            coalescer.complete(first);
            Check.equal(null, coalescer.complete(first), "late duplicate of the first");
            Check.isTrue(coalescer.isInFlight(second), "second still in flight");
            coalescer.forget(7);
            Check.isTrue(!coalescer.isInFlight(second), "forgotten");
            Check.equal(null, coalescer.complete(second), "completion after forget");
        });
    }
}
