package com.droiddeck.launcher.hq;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;

/** Run with javac/java -ea; no Android device or downloaded test runner needed. */
public final class RuntimeRulesTest {
    public static void main(String[] args) throws Exception {
        String gameUrl = "steam://rungameid/1966720";
        assert RuntimeRules.hqGameControls(false, gameUrl, null) : "Cold launch must choose HQ controls before the service assigns its URL";
        assert RuntimeRules.hqGameControls(false, null, gameUrl) : "Resume uses the running game session";
        assert !RuntimeRules.hqGameControls(true, gameUrl, gameUrl) : "Installer always keeps setup controls";
        assert !RuntimeRules.hqGameControls(false, null, null);
        assert !RuntimeRules.hqGameControls(false, "steam://store", null);
        assert !RuntimeRules.hqGameControls(false, gameUrl, "steam://store") : "Active session takes priority over a stale request";
        assert RuntimeRules.sessionStopped(false, "idle", false, false, false);
        assert RuntimeRules.sessionStopped(false, "failed", false, false, false);
        assert !RuntimeRules.sessionStopped(false, "idle", false, false, true) : "Accepted activity launch must block writes";
        for (String phase : new String[] { "stopping", "preparing", "installing_runtime", "starting_compositor", "starting_guest", "starting_steam", "ready", "suspended", "unknown" }) {
            assert !RuntimeRules.sessionStopped(false, phase, false, false, false) : phase + " must block mod writes";
        }
        assert !RuntimeRules.sessionStopped(true, "idle", false, false, false);
        assert !RuntimeRules.sessionStopped(false, "idle", true, false, false);
        assert !RuntimeRules.sessionStopped(false, "idle", false, true, false);
        assert "winhttp=n,b".equals(RuntimeRules.withWinHttp(null));
        assert "wininet=b;d3d11=n,b;winhttp=n,b".equals(
            RuntimeRules.withWinHttp("winhttp,wininet=b;d3d11=n,b;WINHTTP=b"));
        assert "dxgi=n;winhttp=n,b".equals(RuntimeRules.withWinHttp("dxgi=n;winhttp=n,b"));

        File temp = Files.createTempDirectory("hq-runtime-rules-").toFile();
        try {
            File root = new File(temp, "files");
            File linux = new File(root, "linuxfs");
            File game = new File(linux, "root/Steam/steamapps/common/Lethal Company");
            assert game.mkdirs();
            assert game.getCanonicalFile().equals(RuntimeRules.internalDirectory(root, linux, game));
            rejected(root, linux, root);
            rejected(root, linux, linux);
            rejected(root, linux, new File(root, "linuxfs-other"));
            rejected(root, new File(temp, "external"), game);
            rejected(root, linux, new File(linux, "../outside"));
            rejected(root, linux, new File(linux, "missing"));
        } finally {
            try (var paths = Files.walk(temp.toPath())) {
                for (var item : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.delete(item);
            }
        }
        System.out.println("RuntimeRules: cold/resumed game controls, installer isolation, overrides and private-directory boundaries passed");
    }

    private static void rejected(File root, File linux, File candidate) throws Exception {
        try {
            RuntimeRules.internalDirectory(root, linux, candidate);
            throw new AssertionError("Accepted unsafe directory: " + candidate);
        } catch (IOException expected) { }
    }
}
