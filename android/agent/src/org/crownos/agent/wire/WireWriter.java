package org.crownos.agent.wire;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Appends little-endian payload fields. */
final class WireWriter {
    private static final int MAX_U16 = 0xFFFF;
    private static final int INITIAL_CAPACITY = 64;

    private byte[] bytes = new byte[INITIAL_CAPACITY];
    private int size;

    WireWriter u16(int value) {
        reserve(2);
        bytes[size++] = (byte) value;
        bytes[size++] = (byte) (value >>> 8);
        return this;
    }

    WireWriter u32(int value) {
        reserve(4);
        bytes[size++] = (byte) value;
        bytes[size++] = (byte) (value >>> 8);
        bytes[size++] = (byte) (value >>> 16);
        bytes[size++] = (byte) (value >>> 24);
        return this;
    }

    WireWriter bool(boolean value) {
        return u32(value ? 1 : 0);
    }

    WireWriter str(String value) throws WireException {
        byte[] utf8 = value.getBytes(StandardCharsets.UTF_8);
        if (utf8.length > MAX_U16) {
            throw new WireException(WireException.Kind.STRING_TOO_LONG, "a string field is longer than 65535 bytes");
        }
        u16(utf8.length);
        reserve(utf8.length);
        System.arraycopy(utf8, 0, bytes, size, utf8.length);
        size += utf8.length;
        return this;
    }

    WireWriter u32List(int[] values) throws WireException {
        if (values.length > MAX_U16) {
            throw new WireException(WireException.Kind.LIST_TOO_LONG, "a list field has more than 65535 items");
        }
        u16(values.length);
        for (int value : values) {
            u32(value);
        }
        return this;
    }

    int size() {
        return size;
    }

    void copyTo(byte[] target, int offset) {
        System.arraycopy(bytes, 0, target, offset, size);
    }

    private void reserve(int count) {
        if (size + count > bytes.length) {
            bytes = Arrays.copyOf(bytes, Math.max(bytes.length * 2, size + count));
        }
    }
}
