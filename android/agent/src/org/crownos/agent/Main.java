package org.crownos.agent;

import android.os.Process;
import android.system.Os;

import java.util.concurrent.CountDownLatch;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.util.AgentLog;

/**
 * crown-agent: run as root by init through
 * {@code CLASSPATH=/system/framework/crown-agent.jar app_process /system/bin org.crownos.agent.Main}.
 */
public final class Main {
    private static final String PROCESS_NAME = "crown-agent";
    /** Files the agent creates are private to their owner. */
    private static final int PRIVATE_FILES_UMASK = 077;
    private static final int EXIT_FAILURE = 1;

    private Main() {
    }

    public static void main(String[] arguments) {
        Os.umask(PRIVATE_FILES_UMASK);
        try {
            Hidden.callStatic(Process.class, "setArgV0", Hidden.types(String.class), PROCESS_NAME);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot rename the process: " + e.getMessage());
        }
        try {
            new Agent().run();
            new CountDownLatch(1).await();
        } catch (RuntimeException | LinkageError e) {
            AgentLog.error("crown-agent failed to start", e);
            System.exit(EXIT_FAILURE);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}
