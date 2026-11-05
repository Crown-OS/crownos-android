package org.crownos.agent.wire;

/** Encodes and decodes whole agent-wire v1 records. */
public final class WireCodec {
    /** Bumped for any change an older peer cannot read. */
    public static final int WIRE_VERSION = 1;
    /** Largest {@code len} a peer accepts. */
    public static final int MAX_RECORD_LEN = 64 * 1024;
    /** Bytes of {@code len} itself. */
    public static final int LENGTH_BYTES = 4;
    /** Header bytes after {@code len}: {@code type} and {@code flags}. */
    static final int HEADER_AFTER_LENGTH = 4;

    private WireCodec() {
    }

    /** Serializes {@code record} with its header. */
    public static byte[] encode(Record record) throws WireException {
        WireWriter payload = new WireWriter();
        record.writePayload(payload);
        int length = HEADER_AFTER_LENGTH + payload.size();
        checkLength(length);
        WireWriter header = new WireWriter().u32(length).u16(record.type()).u16(0);
        byte[] bytes = new byte[LENGTH_BYTES + length];
        header.copyTo(bytes, 0);
        payload.copyTo(bytes, LENGTH_BYTES + HEADER_AFTER_LENGTH);
        return bytes;
    }

    /** Rejects a {@code len} field a peer must not accept. */
    static void checkLength(int length) throws WireException {
        if (length < 0 || length > MAX_RECORD_LEN) {
            throw new WireException(WireException.Kind.TOO_LONG,
                    "record of " + Integer.toUnsignedString(length) + " bytes exceeds the " + MAX_RECORD_LEN
                            + " byte limit");
        }
        if (length < HEADER_AFTER_LENGTH) {
            throw WireException.malformed(0);
        }
    }

    /** Decodes the {@code length} bytes after {@code len}: type, flags and payload. */
    static Record decodeBody(byte[] body, int length) throws WireException {
        int type = (body[0] & 0xFF) | (body[1] & 0xFF) << 8;
        WireReader in = new WireReader(type, body, HEADER_AFTER_LENGTH, length - HEADER_AFTER_LENGTH);
        Record record = parse(type, in);
        in.requireEnd();
        return record;
    }

    private static Record parse(int type, WireReader in) throws WireException {
        switch (type) {
            case Hello.TYPE:
                return Hello.read(in);
            case HwcCommand.SetSize.TYPE:
                return HwcCommand.SetSize.read(in);
            case HwcCommand.Focus.TYPE:
                return HwcCommand.Focus.read(in);
            case HwcCommand.Close.TYPE:
                return HwcCommand.Close.read(in);
            case HwcCommand.Back.TYPE:
                return HwcCommand.Back.read(in);
            case HwcCommand.Visibility.TYPE:
                return HwcCommand.Visibility.read(in);
            case HwcEvent.TaskInfo.TYPE:
                return HwcEvent.TaskInfo.read(in);
            case HwcEvent.SlotApplied.TYPE:
                return HwcEvent.SlotApplied.read(in);
            case HwcEvent.TaskRemoved.TYPE:
                return HwcEvent.TaskRemoved.read(in);
            case HwcEvent.Activate.TYPE:
                return HwcEvent.Activate.read(in);
            case DaemonCommand.Launch.TYPE:
                return DaemonCommand.Launch.read(in);
            case DaemonCommand.ListTasks.TYPE:
                return new DaemonCommand.ListTasks();
            case DaemonCommand.QueryKeepAlive.TYPE:
                return new DaemonCommand.QueryKeepAlive();
            case DaemonCommand.QueryGsfId.TYPE:
                return new DaemonCommand.QueryGsfId();
            case DaemonCommand.Shutdown.TYPE:
                return new DaemonCommand.Shutdown();
            case DaemonCommand.OpenUri.TYPE:
                return DaemonCommand.OpenUri.read(in);
            case DaemonCommand.InstallPackage.TYPE:
                return DaemonCommand.InstallPackage.read(in);
            case DaemonEvent.Ready.TYPE:
                return DaemonEvent.Ready.read(in);
            case DaemonEvent.TaskOpened.TYPE:
                return DaemonEvent.TaskOpened.read(in);
            case DaemonEvent.TaskClosed.TYPE:
                return DaemonEvent.TaskClosed.read(in);
            case DaemonEvent.Tasks.TYPE:
                return DaemonEvent.Tasks.read(in);
            case DaemonEvent.KeepAlive.TYPE:
                return DaemonEvent.KeepAlive.read(in);
            case DaemonEvent.GsfId.TYPE:
                return DaemonEvent.GsfId.read(in);
            case DaemonEvent.LaunchFailed.TYPE:
                return DaemonEvent.LaunchFailed.read(in);
            case DaemonEvent.InstallResult.TYPE:
                return DaemonEvent.InstallResult.read(in);
            default:
                throw new WireException(WireException.Kind.UNKNOWN_TYPE,
                        String.format("unknown record type 0x%04x", type));
        }
    }
}
