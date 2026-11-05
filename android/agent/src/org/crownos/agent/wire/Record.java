package org.crownos.agent.wire;

/** One agent-wire v1 record, framed as {@code len: u32, type: u16, flags: u16, payload}. */
public abstract class Record {
    Record() {
    }

    /** The wire type code. */
    public abstract int type();

    abstract void writePayload(WireWriter out) throws WireException;
}
