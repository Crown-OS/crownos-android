package org.crownos.agent.wire;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;

/** Reads little-endian payload fields. Every getter rejects a short payload as malformed. */
final class WireReader {
    private final int type;
    private final byte[] bytes;
    private int position;
    private final int limit;

    WireReader(int type, byte[] bytes, int offset, int length) {
        this.type = type;
        this.bytes = bytes;
        this.position = offset;
        this.limit = offset + length;
    }

    int u16() throws WireException {
        require(2);
        int value = (bytes[position] & 0xFF) | (bytes[position + 1] & 0xFF) << 8;
        position += 2;
        return value;
    }

    int u32() throws WireException {
        require(4);
        int value = (bytes[position] & 0xFF)
                | (bytes[position + 1] & 0xFF) << 8
                | (bytes[position + 2] & 0xFF) << 16
                | (bytes[position + 3] & 0xFF) << 24;
        position += 4;
        return value;
    }

    boolean bool() throws WireException {
        int value = u32();
        if (value != 0 && value != 1) {
            throw WireException.malformed(type);
        }
        return value == 1;
    }

    String str() throws WireException {
        int length = u16();
        require(length);
        try {
            String value = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(bytes, position, length))
                    .toString();
            position += length;
            return value;
        } catch (CharacterCodingException e) {
            throw WireException.malformed(type);
        }
    }

    int[] u32List() throws WireException {
        int[] values = new int[u16()];
        for (int index = 0; index < values.length; index++) {
            values[index] = u32();
        }
        return values;
    }

    void requireEnd() throws WireException {
        if (position != limit) {
            throw WireException.malformed(type);
        }
    }

    private void require(int count) throws WireException {
        if (limit - position < count) {
            throw WireException.malformed(type);
        }
    }
}
