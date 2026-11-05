package org.crownos.agent.policy;

import org.crownos.agent.testing.Check;
import org.crownos.agent.testing.TestRunner;

public final class PolicyTest {
    private PolicyTest() {
    }

    private static void uris(boolean allowed, String... uris) {
        for (String uri : uris) {
            Check.equal(allowed, UriPolicy.isAllowed(uri), uri);
        }
    }

    private static void inbox(String expected, String... paths) {
        for (String path : paths) {
            Check.equal(expected, InboxPolicy.fileName(path), String.valueOf(path));
        }
    }

    public static void run(TestRunner runner) {
        runner.test("policy: market and https links are allowed", () -> uris(true,
                "market://details?id=com.whatsapp",
                "market://search?q=maps",
                "https://play.google.com/store/apps/details?id=com.whatsapp",
                "HTTPS://example.org/path?q=1#top"));

        runner.test("policy: every other scheme and malformed links are refused", () -> uris(false,
                null, "", "http://example.org", "intent://scan/#Intent;scheme=zxing;end", "file:///data/system",
                "content://settings/global", "javascript:alert(1)", "market:", "https:", "https:///path",
                "https://exa mple.org", "https://example.org/\n", "//example.org", "details?id=x"));

        runner.test("policy: inbox files are bare names or absolute paths directly under the inbox", () -> {
            inbox("app.apk", "app.apk", "/data/crownos/inbox/app.apk");
            inbox("My App (1).apk", "My App (1).apk");
            inbox(".hidden.apk", ".hidden.apk");
        });

        runner.test("policy: path traversal and other names are refused", () -> inbox(null,
                null, "", ".", "..", "../app.apk", "sub/app.apk", "/data/crownos/inbox/", "/data/crownos/inbox/..",
                "/data/crownos/inbox/../agent.sock", "/data/crownos/inbox//app.apk", "/data/app/base.apk",
                "/data/crownos/inbox/sub/app.apk", "app\u0000.apk", "app\n.apk", new String(new char[256]).replace(
                        '\u0000', 'a')));

        runner.test("policy: package names follow the package parser", () -> {
            Check.isTrue(PackageNames.isValid("com.whatsapp"), "two segments");
            Check.isTrue(PackageNames.isValid("org.crownos.android.home_2"), "digits and underscores");
            for (String invalid : new String[] {null, "", "android", "com.", ".com", "com..x", "1com.x", "com.1x",
                "com.what sapp", "com/whatsapp", "com.whatsapp;rm"}) {
                Check.isTrue(!PackageNames.isValid(invalid), "invalid: " + invalid);
            }
        });
    }
}
