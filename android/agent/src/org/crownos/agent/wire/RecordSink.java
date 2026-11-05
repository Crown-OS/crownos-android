package org.crownos.agent.wire;

/** Somewhere records can be sent. Sending never blocks the caller. */
public interface RecordSink {
    void send(Record record);
}
