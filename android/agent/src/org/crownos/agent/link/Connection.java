package org.crownos.agent.link;

import android.net.LocalSocket;

import java.io.IOException;
import java.io.OutputStream;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.atomic.AtomicBoolean;

import org.crownos.agent.util.AgentLog;
import org.crownos.agent.wire.Record;
import org.crownos.agent.wire.RecordInput;
import org.crownos.agent.wire.WireCodec;
import org.crownos.agent.wire.WireException;

/**
 * One peer connection with a reader and a writer thread. Sending only queues, so a slow peer never
 * blocks the agent loop; a peer that stops reading is dropped once its queue fills.
 */
final class Connection {
    /** Hears records and the end of the connection, on the reader thread. */
    interface Receiver {
        void onRecord(Connection connection, Record record);

        void onClosed(Connection connection);
    }

    private static final int OUTBOX_RECORDS = 1024;

    private final String name;
    private final LocalSocket socket;
    private final Receiver receiver;
    private final BlockingQueue<byte[]> outbox = new ArrayBlockingQueue<>(OUTBOX_RECORDS);
    private final AtomicBoolean closed = new AtomicBoolean();
    private final Thread writer;

    Connection(String name, LocalSocket socket, Receiver receiver) {
        this.name = name;
        this.socket = socket;
        this.receiver = receiver;
        this.writer = new Thread(this::writeLoop, name + "-writer");
    }

    void start() {
        new Thread(this::readLoop, name + "-reader").start();
        writer.start();
    }

    void send(Record record) {
        if (closed.get()) {
            return;
        }
        byte[] bytes;
        try {
            bytes = WireCodec.encode(record);
        } catch (WireException e) {
            AgentLog.warn(name + ": cannot encode a record: " + e.getMessage());
            return;
        }
        if (!outbox.offer(bytes)) {
            AgentLog.warn(name + ": peer stopped reading, dropping the connection");
            close();
        }
    }

    void close() {
        if (!closed.compareAndSet(false, true)) {
            return;
        }
        try {
            socket.shutdownInput();
            socket.shutdownOutput();
        } catch (IOException ignored) {
            AgentLog.info(name + ": socket already shut down");
        }
        try {
            socket.close();
        } catch (IOException e) {
            AgentLog.warn(name + ": close failed: " + e.getMessage());
        }
        writer.interrupt();
        receiver.onClosed(this);
    }

    private void readLoop() {
        try {
            RecordInput input = new RecordInput(socket.getInputStream());
            Record record;
            while ((record = input.read()) != null) {
                receiver.onRecord(this, record);
            }
        } catch (IOException | WireException e) {
            if (!closed.get()) {
                AgentLog.warn(name + ": dropping the connection: " + e.getMessage());
            }
        } finally {
            close();
        }
    }

    private void writeLoop() {
        try {
            OutputStream output = socket.getOutputStream();
            while (!closed.get()) {
                output.write(outbox.take());
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } catch (IOException e) {
            if (!closed.get()) {
                AgentLog.warn(name + ": write failed: " + e.getMessage());
            }
        } finally {
            close();
        }
    }
}
