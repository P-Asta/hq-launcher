package com.droiddeck.launcher.hq;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/** HQ's keyboard/mouse touch state. Joystick coordinates are normalized; look coordinates are pixels. */
public final class GameTouchInput {
    public enum Action { JOYSTICK, LOOK, INTERACT, DROP, UTILITY, SCROLL_UP, SCROLL_DOWN, ESCAPE, CONTROL, SHIFT_TOGGLE, RIGHT_CLICK, SPACE }
    public interface Sink {
        void key(int code, boolean down);
        void relative(float x, float y);
        void absolute(float x, float y);
        void mouse(int button, boolean down);
        void wheel(int steps);
    }
    private static final int LEFT = 272, RIGHT = 273;
    private final Sink sink;
    private final boolean utility;
    private final float slop;
    private final Map<Integer, Finger> fingers = new LinkedHashMap<>();
    private final Map<Integer, Integer> keys = new HashMap<>();
    private final Map<Integer, Integer> mouseButtons = new HashMap<>();
    private final Finger sprint = new Finger(Action.SHIFT_TOGGLE, 0, 0, 0);
    private float sensitivity = 1f;
    private int lookId = -1;
    private boolean grabbed;

    private static final class Finger {
        final Action action;
        final long downAt;
        final Set<Integer> keys = new HashSet<>();
        final Set<Integer> buttons = new HashSet<>();
        float x, y, startX, startY;
        boolean moved, gestures = true, leftHeld;
        long nextScroll;
        Finger(Action action, float x, float y, long time) {
            this.action = action; this.x = this.startX = x; this.y = this.startY = y;
            downAt = time; nextScroll = time + 350;
        }
    }
    public GameTouchInput(Sink sink, int gameVersion, float slop) {
        this.sink = sink; utility = gameVersion >= 80; this.slop = slop;
    }
    public boolean utilityEnabled() { return utility; }
    public boolean shiftToggled() { return !sprint.keys.isEmpty(); }
    public void setSensitivity(float value) { sensitivity = Float.isFinite(value) ? Math.max(.25f, Math.min(3f, value)) : 1f; }
    public void setGrabbed(boolean value) {
        if (grabbed == value) return;
        cancel();
        grabbed = value;
    }
    public void down(int id, Action action, float x, float y, long time) {
        if (fingers.containsKey(id) || action == Action.UTILITY && !utility) return;
        Finger finger = new Finger(action, x, y, time);
        fingers.put(id, finger);
        switch (action) {
            case JOYSTICK: joystick(finger, x, y); break;
            case INTERACT: hold(finger, 18, true); break;
            case DROP: hold(finger, 34, true); break;
            case UTILITY: hold(finger, 15, true); break;
            case ESCAPE: hold(finger, 1, true); break;
            case CONTROL: hold(finger, 29, true); break;
            case SPACE: hold(finger, 57, true); break;
            case SHIFT_TOGGLE: hold(sprint, 42, !shiftToggled()); break;
            case RIGHT_CLICK: holdMouse(finger, RIGHT, true); break;
            case SCROLL_UP: sink.wheel(-1); break;
            case SCROLL_DOWN: sink.wheel(1); break;
            case LOOK:
                if (lookId < 0) {
                    lookId = id;
                    if (!grabbed) sink.absolute(x, y);
                } else {
                    fingers.get(lookId).gestures = false;
                    finger.gestures = false;
                }
                break;
        }
    }
    public void move(int id, float x, float y, long time) {
        Finger finger = fingers.get(id);
        if (finger == null) return;
        float dx = x - finger.x, dy = y - finger.y;
        finger.x = x; finger.y = y;
        if (finger.action == Action.JOYSTICK) { joystick(finger, x, y); return; }
        if (finger.action != Action.LOOK || id != lookId) return;
        if (Math.hypot(x - finger.startX, y - finger.startY) > slop) finger.moved = true;
        if (grabbed) sink.relative(dx * sensitivity, dy * sensitivity);
        else {
            if (finger.gestures && finger.moved && !finger.leftHeld) {
                sink.absolute(finger.startX, finger.startY);
                holdMouse(finger, LEFT, true); finger.leftHeld = true;
            }
            sink.absolute(x, y);
        }
    }
    public void up(int id, long time) {
        Finger finger = fingers.remove(id);
        if (finger == null) return;
        release(finger);
        if (finger.action != Action.LOOK || id != lookId) return;
        if (!finger.leftHeld && finger.gestures && !finger.moved && time >= finger.downAt &&
                time - finger.downAt <= (grabbed ? 150 : 300)) {
            sink.mouse(LEFT, true); sink.mouse(LEFT, false);
        }
        lookId = -1;
        for (Map.Entry<Integer, Finger> other : fingers.entrySet()) {
            if (other.getValue().action != Action.LOOK) continue;
            lookId = other.getKey();
            other.getValue().gestures = false;
            break;
        }
    }
    public void tick(long time) {
        for (Map.Entry<Integer, Finger> entry : fingers.entrySet()) {
            Finger finger = entry.getValue();
            if ((finger.action == Action.SCROLL_UP || finger.action == Action.SCROLL_DOWN) && time >= finger.nextScroll) {
                sink.wheel(finger.action == Action.SCROLL_UP ? -1 : 1);
                finger.nextScroll = time + 120;
            }
            if (entry.getKey() == lookId && grabbed && finger.gestures && !finger.moved &&
                    !finger.leftHeld && time - finger.downAt >= 300) {
                holdMouse(finger, LEFT, true); finger.leftHeld = true;
            }
        }
    }
    public void cancel() {
        cancelTouches();
        release(sprint);
    }
    public void cancelTouches() {
        for (Finger finger : fingers.values()) release(finger);
        fingers.clear(); lookId = -1;
    }
    private void release(Finger finger) {
        for (int key : new HashSet<>(finger.keys)) hold(finger, key, false);
        for (int button : new HashSet<>(finger.buttons)) holdMouse(finger, button, false);
        // Keep leftHeld for up(): a held click must never also emit a tap.
    }
    private void holdMouse(Finger finger, int button, boolean down) {
        if (down) {
            if (!finger.buttons.add(button)) return;
            int count = mouseButtons.getOrDefault(button, 0);
            mouseButtons.put(button, count + 1);
            if (count == 0) sink.mouse(button, true);
        } else {
            if (!finger.buttons.remove(button)) return;
            int count = mouseButtons.getOrDefault(button, 1) - 1;
            if (count == 0) { mouseButtons.remove(button); sink.mouse(button, false); }
            else mouseButtons.put(button, count);
        }
    }
    private void hold(Finger finger, int key, boolean down) {
        if (down) {
            if (!finger.keys.add(key)) return;
            int count = keys.getOrDefault(key, 0);
            keys.put(key, count + 1);
            if (count == 0) sink.key(key, true);
        } else {
            if (!finger.keys.remove(key)) return;
            int count = keys.getOrDefault(key, 1) - 1;
            if (count == 0) { keys.remove(key); sink.key(key, false); }
            else keys.put(key, count);
        }
    }
    private void joystick(Finger finger, float x, float y) {
        Set<Integer> next = new HashSet<>();
        if (Math.hypot(x, y) > .35) {
            if (Math.abs(x) >= Math.abs(y) * .41421356f) next.add(x < 0 ? 30 : 32);
            if (Math.abs(y) >= Math.abs(x) * .41421356f) next.add(y < 0 ? 17 : 31);
        }
        for (int key : new HashSet<>(finger.keys)) if (!next.contains(key)) hold(finger, key, false);
        for (int key : next) hold(finger, key, true);
    }
}
