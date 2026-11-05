package org.crownos.agent.util;

import java.util.concurrent.Executor;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;

/**
 * One named thread that runs work in order. A failure in one piece of work is logged and never
 * stops the thread, so a missing hidden API cannot take the agent down.
 */
public final class Loop implements Executor {
    private final String name;
    private final ScheduledExecutorService executor;

    public Loop(String name) {
        this.name = name;
        this.executor = Executors.newSingleThreadScheduledExecutor(work -> new Thread(work, name));
    }

    @Override
    public void execute(Runnable work) {
        executor.execute(guarded(work));
    }

    public ScheduledFuture<?> schedule(Runnable work, long delayMillis) {
        return executor.schedule(guarded(work), delayMillis, TimeUnit.MILLISECONDS);
    }

    public void every(Runnable work, long periodMillis) {
        executor.scheduleWithFixedDelay(guarded(work), periodMillis, periodMillis, TimeUnit.MILLISECONDS);
    }

    private Runnable guarded(Runnable work) {
        return () -> {
            try {
                work.run();
            } catch (RuntimeException | LinkageError failure) {
                AgentLog.error("work on " + name + " failed", failure);
            }
        };
    }
}
