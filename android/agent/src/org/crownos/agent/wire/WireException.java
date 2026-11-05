package org.crownos.agent.wire;

/** A record that cannot be encoded or decoded. A peer drops the connection on any of these. */
public final class WireException extends Exception {
    private static final long serialVersionUID = 1L;

    /** Why the record was rejected. */
    public enum Kind {
        TOO_LONG,
        UNKNOWN_TYPE,
        MALFORMED,
        STRING_TOO_LONG,
        LIST_TOO_LONG
    }

    public final Kind kind;

    WireException(Kind kind, String message) {
        super(message);
        this.kind = kind;
    }

    static WireException malformed(int type) {
        return new WireException(Kind.MALFORMED, String.format("record type 0x%04x is malformed", type));
    }
}
