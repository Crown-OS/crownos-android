package org.crownos.agent.testing;

import java.nio.file.Path;
import java.nio.file.Paths;

import org.crownos.agent.atlas.AtlasTest;
import org.crownos.agent.keepalive.KeepAliveProbeTest;
import org.crownos.agent.policy.PolicyTest;
import org.crownos.agent.sizing.ResizeCoalescerTest;
import org.crownos.agent.wire.WireCodecTest;

/** Runs every host test and exits non-zero when one fails. Usage: TestRunner <agent-wire-vectors.txt>. */
public final class TestRunner {
    private int passed;
    private int failed;

    public void test(String name, Check.Body body) {
        try {
            body.run();
            passed++;
            System.out.println("PASS " + name);
        } catch (Throwable failure) {
            failed++;
            System.out.println("FAIL " + name + ": " + failure);
            failure.printStackTrace(System.out);
        }
    }

    public static void main(String[] arguments) {
        if (arguments.length != 1) {
            System.err.println("usage: TestRunner <agent-wire-vectors.txt>");
            System.exit(2);
        }
        Path vectors = Paths.get(arguments[0]);
        TestRunner runner = new TestRunner();
        WireCodecTest.run(runner, vectors);
        AtlasTest.run(runner);
        ResizeCoalescerTest.run(runner);
        PolicyTest.run(runner);
        KeepAliveProbeTest.run(runner);
        System.out.println(runner.passed + " passed, " + runner.failed + " failed");
        System.exit(runner.failed == 0 ? 0 : 1);
    }
}
