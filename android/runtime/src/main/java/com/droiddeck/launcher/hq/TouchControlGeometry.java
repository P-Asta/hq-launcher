package com.droiddeck.launcher.hq;

/** Coordinates are normalized to the usable window, so layouts survive size changes. */
public final class TouchControlGeometry {
    private TouchControlGeometry() {}
    private static float finite(float value, float fallback) { return Float.isFinite(value) ? value : fallback; }
    private static float clamp(float value, float low, float high) { return Math.max(low, Math.min(high, value)); }
    public static float scale(float value) { return clamp(finite(value, 1), .75f, 1.75f); }
    public static float normalized(float value, float low, float high) {
        return high > low ? clamp(finite((value-low)/(high-low), .5f), 0, 1) : .5f;
    }
    public static float[] bounds(float left, float top, float right, float bottom, float x, float y, float size) {
        left = finite(left, 0); top = finite(top, 0);
        right = Math.max(left+1, finite(right, left+1)); bottom = Math.max(top+1, finite(bottom, top+1));
        float edge = clamp(finite(size, 48), 1, Math.min(right-left, bottom-top));
        float cx = clamp(left + clamp(finite(x, .5f), 0, 1)*(right-left), left+edge/2, right-edge/2);
        float cy = clamp(top + clamp(finite(y, .5f), 0, 1)*(bottom-top), top+edge/2, bottom-edge/2);
        return new float[] { cx-edge/2, cy-edge/2, cx+edge/2, cy+edge/2 };
    }
}
