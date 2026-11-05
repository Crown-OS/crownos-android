package org.crownos.agent;

import java.util.concurrent.CountDownLatch;

final class Lifetime {
    private Lifetime() {
    }

    static void sleep(long millis) {
        try {
            Thread.sleep(millis);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("interrupted", e);
        }
    }

    static void hold(int seconds) {
        if (seconds > 0) {
            sleep(seconds * 1000L);
            return;
        }
        try {
            new CountDownLatch(1).await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}
