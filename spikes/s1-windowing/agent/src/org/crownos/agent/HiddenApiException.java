package org.crownos.agent;

final class HiddenApiException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    HiddenApiException(String message, Throwable cause) {
        super(cause == null ? message : message + ": " + cause, cause);
    }
}
