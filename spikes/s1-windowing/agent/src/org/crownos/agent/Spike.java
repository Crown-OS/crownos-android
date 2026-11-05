package org.crownos.agent;

public final class Spike {
    private static final int EXIT_FAILURE = 1;

    private Spike() {
    }

    public static void main(String[] arguments) {
        try {
            Commands.run(arguments);
        } catch (RuntimeException e) {
            Events.error(e);
            System.exit(EXIT_FAILURE);
        }
        System.exit(0);
    }
}
