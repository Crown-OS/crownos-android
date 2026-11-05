package org.crownos.agent.platform;

/** A hidden framework class, method or field is missing or failed on this image. */
public final class HiddenApiException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    public HiddenApiException(String message, Throwable cause) {
        super(cause == null ? message : message + ": " + cause, cause);
    }
}
