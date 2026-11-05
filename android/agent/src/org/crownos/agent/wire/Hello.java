package org.crownos.agent.wire;

/** Both directions, the first record on every connection. */
public final class Hello extends Record {
    public static final int TYPE = 0x0001;

    public final int wire;
    public final String bundle;

    public Hello(int wire, String bundle) {
        this.wire = wire;
        this.bundle = bundle;
    }

    @Override
    public int type() {
        return TYPE;
    }

    @Override
    void writePayload(WireWriter out) throws WireException {
        out.u32(wire).str(bundle);
    }

    static Hello read(WireReader in) throws WireException {
        return new Hello(in.u32(), in.str());
    }
}
