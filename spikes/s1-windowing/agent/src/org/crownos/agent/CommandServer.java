package org.crownos.agent;

import android.os.SystemClock;
import android.system.ErrnoException;
import android.system.Os;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileOutputStream;
import java.io.FileReader;
import java.io.IOException;
import java.io.PrintStream;

final class CommandServer {
    private static final int FIFO_MODE = 0666;
    private static final int OUTPUT_MODE = 0644;

    private CommandServer() {
    }

    static void serve(String pathPrefix) {
        String inputPath = pathPrefix + ".in";
        String outputPath = pathPrefix + ".out";
        try (PrintStream output = new PrintStream(new FileOutputStream(outputPath, true), true)) {
            Os.chmod(outputPath, OUTPUT_MODE);
            createFifo(inputPath);
            Events.redirect(output);
            Events.emit("serve.ready", "uid=" + Os.getuid() + " pid=" + Os.getpid() + " input=" + inputPath
                    + " uptime_ms=" + SystemClock.uptimeMillis());
            while (true) {
                readCommands(inputPath);
            }
        } catch (IOException | ErrnoException e) {
            throw new IllegalStateException("command server failed: " + e.getMessage(), e);
        }
    }

    private static void createFifo(String path) throws ErrnoException {
        File fifo = new File(path);
        if (fifo.exists() && !fifo.delete()) {
            throw new IllegalStateException("cannot replace " + path);
        }
        Os.mkfifo(path, FIFO_MODE);
        Os.chmod(path, FIFO_MODE);
    }

    private static void readCommands(String inputPath) throws IOException {
        try (BufferedReader reader = new BufferedReader(new FileReader(inputPath))) {
            String line;
            while ((line = reader.readLine()) != null) {
                execute(line.trim());
            }
        }
    }

    private static void execute(String line) {
        if (line.isEmpty()) {
            return;
        }
        String[] arguments = line.split("\\s+");
        Events.emit("serve.command", "line=\"" + line + "\"");
        Runnable job = () -> {
            try {
                Commands.run(arguments);
            } catch (RuntimeException e) {
                Events.error(e);
            }
        };
        if (Commands.isLongRunning(arguments[0])) {
            new Thread(job, "crown-" + arguments[0]).start();
        } else {
            job.run();
        }
    }
}
