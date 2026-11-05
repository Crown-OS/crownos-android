package org.crownos.agent.wire;

import java.io.DataInputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;

/** Reads whole records from a byte stream, reusing one buffer per connection. */
public final class RecordInput {
    private final DataInputStream in;
    private final byte[] length = new byte[WireCodec.LENGTH_BYTES];
    private final byte[] body = new byte[WireCodec.MAX_RECORD_LEN];

    public RecordInput(InputStream in) {
        this.in = new DataInputStream(in);
    }

    /**
     * Blocks for the next record. Returns {@code null} when the stream ends between records.
     *
     * @throws EOFException when the stream ends inside a record
     * @throws WireException when the record is invalid
     */
    public Record read() throws IOException, WireException {
        int first = in.read();
        if (first < 0) {
            return null;
        }
        length[0] = (byte) first;
        in.readFully(length, 1, length.length - 1);
        int recordLength = (length[0] & 0xFF) | (length[1] & 0xFF) << 8 | (length[2] & 0xFF) << 16
                | (length[3] & 0xFF) << 24;
        WireCodec.checkLength(recordLength);
        in.readFully(body, 0, recordLength);
        return WireCodec.decodeBody(body, recordLength);
    }
}
