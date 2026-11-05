package org.crownos.agent.testing;

/** Lowercase hex for test vectors. */
public final class Hex {
    private static final char[] DIGITS = "0123456789abcdef".toCharArray();

    private Hex() {
    }

    public static String encode(byte[] bytes) {
        char[] text = new char[bytes.length * 2];
        for (int index = 0; index < bytes.length; index++) {
            text[index * 2] = DIGITS[(bytes[index] >> 4) & 0xF];
            text[index * 2 + 1] = DIGITS[bytes[index] & 0xF];
        }
        return new String(text);
    }

    public static byte[] decode(String text) {
        byte[] bytes = new byte[text.length() / 2];
        for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) Integer.parseInt(text.substring(index * 2, index * 2 + 2), 16);
        }
        return bytes;
    }
}
