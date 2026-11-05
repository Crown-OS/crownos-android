package org.crownos.agent.keepalive;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.crownos.agent.testing.Check;
import org.crownos.agent.testing.TestRunner;

public final class KeepAliveProbeTest {
    private KeepAliveProbeTest() {
    }

    private static KeepAliveSignal signal(int reason, boolean active, boolean fails) {
        return new KeepAliveSignal() {
            @Override
            public int reason() {
                return reason;
            }

            @Override
            public String name() {
                return "signal " + reason;
            }

            @Override
            public boolean isActive() {
                if (fails) {
                    throw new IllegalStateException("hidden API missing");
                }
                return active;
            }
        };
    }

    public static void run(TestRunner runner) {
        runner.test("keepalive: active signals set their bits and a failing signal counts as inactive", () -> {
            List<String> failures = new ArrayList<>();
            KeepAliveProbe probe = new KeepAliveProbe(Arrays.asList(
                    signal(KeepAliveReasons.AUDIO, true, false),
                    signal(KeepAliveReasons.CALL, false, false),
                    signal(KeepAliveReasons.INSTALL, true, true),
                    signal(KeepAliveReasons.DOWNLOAD, true, false)),
                    (signal, failure) -> failures.add(signal.name()));
            Check.equal(KeepAliveReasons.AUDIO | KeepAliveReasons.DOWNLOAD, probe.reasons(), "reasons");
            Check.equal(Arrays.asList("signal " + KeepAliveReasons.INSTALL), failures, "failure reported");
        });

        runner.test("keepalive: bits match agent-wire v1", () -> {
            Check.equal(0b11111, KeepAliveReasons.AUDIO | KeepAliveReasons.CALL | KeepAliveReasons.INSTALL
                    | KeepAliveReasons.DOWNLOAD | KeepAliveReasons.FOREGROUND_SERVICE, "five distinct bits");
            Check.equal(8, KeepAliveReasons.DOWNLOAD, "download is bit 3");
        });
    }
}
