package com.droiddeck.launcher.hq;

public final class TouchControlGeometryTest {
    public static void main(String[] args) {
        float[] a = TouchControlGeometry.bounds(40, 10, 800, 400, 1, 1, 100);
        assert a[0] == 700 && a[1] == 300 && a[2] == 800 && a[3] == 400;
        float[] b = TouchControlGeometry.bounds(40, 10, 800, 400, 0, 0, 100);
        assert b[0] == 40 && b[1] == 10 && b[2] == 140 && b[3] == 110;
        float[] c = TouchControlGeometry.bounds(20, 0, 280, 800, .5f, .5f, 600);
        assert c[0] == 20 && c[2] == 280 && c[3]-c[1] == 260 : "Oversized controls stay in portrait safe bounds";
        float[] d = TouchControlGeometry.bounds(0, 0, 800, 400, Float.NaN, Float.POSITIVE_INFINITY, Float.NaN);
        for (float value : d) assert Float.isFinite(value) : "Corrupted saved layout must not hide controls";
        assert d[0] >= 0 && d[1] >= 0 && d[2] <= 800 && d[3] <= 400;
        assert TouchControlGeometry.scale(Float.NaN) == 1f;
        assert TouchControlGeometry.scale(-10) == .75f && TouchControlGeometry.scale(100) == 1.75f;
        assert TouchControlGeometry.normalized(110, 10, 210) == .5f;
        assert TouchControlGeometry.normalized(-10, 10, 210) == 0;
        assert TouchControlGeometry.normalized(1000, 10, 210) == 1;
        System.out.println("TouchControlGeometry: safe area, resize, invalid data and normalized anchors passed");
    }
}
