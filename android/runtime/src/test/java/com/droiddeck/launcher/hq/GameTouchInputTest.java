package com.droiddeck.launcher.hq;

import java.util.ArrayList;
import java.util.List;

/** Pure input regression checks: javac -encoding UTF-8; java -ea. */
public final class GameTouchInputTest {
    static final class Events implements GameTouchInput.Sink {
        final List<String> values = new ArrayList<>();
        public void key(int code, boolean down) { values.add("key:" + code + ":" + down); }
        public void relative(float x, float y) { values.add("relative:" + x + ":" + y); }
        public void absolute(float x, float y) { values.add("absolute:" + x + ":" + y); }
        public void mouse(int button, boolean down) { values.add("mouse:" + button + ":" + down); }
        public void wheel(int steps) { values.add("wheel:" + steps); }
    }

    public static void main(String[] args) {
        Events e = new Events();
        GameTouchInput input = new GameTouchInput(e, 81, 9);
        input.down(3, GameTouchInput.Action.JOYSTICK, 0, 0, 0);
        input.move(3, .8f, -.8f, 1);
        assert e.values.contains("key:17:true") && e.values.contains("key:32:true") : e.values;
        input.down(8, GameTouchInput.Action.INTERACT, 0, 0, 2);
        input.setGrabbed(true);
        assert e.values.contains("key:17:false") && e.values.contains("key:18:false") : "Grab transition releases held keys";
        e.values.clear();
        input.down(3, GameTouchInput.Action.JOYSTICK, 0, 0, 10);
        input.move(3, -.8f, 0, 11);
        input.down(8, GameTouchInput.Action.INTERACT, 0, 0, 12);
        input.down(13, GameTouchInput.Action.LOOK, 100, 100, 13);
        input.move(13, 115, 104, 14);
        assert e.values.contains("relative:15.0:4.0") : e.values;
        assert !e.values.contains("key:30:false") && !e.values.contains("key:18:false") : "Look must not steal joystick/E";
        input.up(8, 20);
        assert e.values.contains("key:18:false") && !e.values.contains("key:30:false");
        input.cancel();
        assert e.values.contains("key:30:false");

        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 20, 30, 100);
        input.up(1, 200);
        assert e.values.equals(List.of("mouse:272:true", "mouse:272:false")) : "Surface short tap is left-click: " + e.values;
        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 20, 30, 300);
        input.tick(599);
        assert e.values.isEmpty();
        input.tick(600);
        assert e.values.equals(List.of("mouse:272:true"));
        input.up(1, 650);
        assert e.values.equals(List.of("mouse:272:true", "mouse:272:false"));

        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 20, 30, 700);
        input.move(1, 60, 30, 720);
        input.up(1, 740);
        assert e.values.equals(List.of("relative:40.0:0.0")) : "Camera swipe must not click";
        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 20, 30, 800);
        input.cancel();
        assert e.values.isEmpty() : "Cancel must not synthesize a tap";

        input.down(1, GameTouchInput.Action.LOOK, 10, 10, 900);
        input.down(7, GameTouchInput.Action.LOOK, 400, 400, 901);
        input.move(7, 402, 400, 910);
        input.up(1, 920);
        input.move(7, 407, 400, 930);
        input.up(7, 940);
        assert e.values.equals(List.of("relative:5.0:0.0")) : "Pointer handoff must not jump or click: " + e.values;

        input.setGrabbed(false);
        e.values.clear();
        input.down(2, GameTouchInput.Action.LOOK, 300, 200, 1000);
        input.up(2, 1080);
        assert e.values.equals(List.of("absolute:300.0:200.0", "mouse:272:true", "mouse:272:false")) : "Menu uses direct tap-left: " + e.values;
        e.values.clear();
        input.down(2, GameTouchInput.Action.LOOK, 300, 200, 1100);
        input.move(2, 330, 200, 1120);
        input.cancel();
        assert e.values.equals(List.of("absolute:300.0:200.0", "absolute:300.0:200.0", "mouse:272:true", "absolute:330.0:200.0", "mouse:272:false")) : "Menu drag must press at the original knob before moving: " + e.values;
        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 10, 10, 1130);
        input.down(7, GameTouchInput.Action.LOOK, 400, 400, 1131);
        input.move(7, 402, 400, 1132);
        input.up(1, 1133);
        input.move(7, 420, 400, 1134);
        input.up(7, 1135);
        assert e.values.stream().noneMatch(value -> value.startsWith("mouse:")) : "GUI pointer handoff must not invent a click: " + e.values;

        for (int version : new int[] {-1, 79, 80, 81}) {
            Events versionEvents = new Events();
            GameTouchInput versionInput = new GameTouchInput(versionEvents, version, 9);
            assert versionInput.utilityEnabled() == (version >= 80);
            versionInput.down(1, GameTouchInput.Action.UTILITY, 0, 0, 0);
            versionInput.up(1, 1);
            assert versionEvents.values.equals(version >= 80 ? List.of("key:15:true", "key:15:false") : List.of()) : version;
        }
        e.values.clear();
        input.down(1, GameTouchInput.Action.INTERACT, 0, 0, 1200);
        input.down(2, GameTouchInput.Action.INTERACT, 0, 0, 1201);
        input.up(1, 1210);
        assert e.values.equals(List.of("key:18:true")) : "A second finger still holds E";
        input.up(2, 1220);
        assert e.values.equals(List.of("key:18:true", "key:18:false"));
        e.values.clear();
        input.down(1, GameTouchInput.Action.SCROLL_UP, 0, 0, 1300);
        input.down(2, GameTouchInput.Action.SCROLL_DOWN, 0, 0, 1300);
        assert e.values.equals(List.of("wheel:-1", "wheel:1")) : "Wayland axis uses negative for up";
        input.tick(1650);
        assert e.values.size() == 4;
        input.cancel();
        input.tick(2000);
        assert e.values.size() == 4 : "Cancel stops scroll repeat";
        input.setGrabbed(true);
        e.values.clear();
        input.down(1, GameTouchInput.Action.CONTROL, 0, 0, 2100);
        input.down(2, GameTouchInput.Action.SPACE, 0, 0, 2101);
        input.down(3, GameTouchInput.Action.RIGHT_CLICK, 0, 0, 2102);
        input.down(4, GameTouchInput.Action.RIGHT_CLICK, 0, 0, 2103);
        input.up(3, 2110);
        assert e.values.equals(List.of("key:29:true", "key:57:true", "mouse:273:true")) : "Right-click stays held by the other finger: " + e.values;
        input.cancel();
        assert e.values.containsAll(List.of("key:29:false", "key:57:false", "mouse:273:false")) : e.values;

        e.values.clear();
        input.down(1, GameTouchInput.Action.SHIFT_TOGGLE, 0, 0, 2200);
        input.up(1, 2210);
        input.cancelTouches();
        assert input.shiftToggled() && e.values.equals(List.of("key:42:true")) : "Sprint survives finger release and the next gesture";
        input.down(2, GameTouchInput.Action.JOYSTICK, 0, -.8f, 2220);
        input.up(2, 2230);
        assert input.shiftToggled() && !e.values.contains("key:42:false");
        input.down(1, GameTouchInput.Action.SHIFT_TOGGLE, 0, 0, 2240);
        input.up(1, 2250);
        assert !input.shiftToggled() && e.values.contains("key:42:false");
        input.down(1, GameTouchInput.Action.SHIFT_TOGGLE, 0, 0, 2260);
        input.up(1, 2270);
        input.cancel();
        assert !input.shiftToggled() && e.values.get(e.values.size()-1).equals("key:42:false") : "Focus/modal cancellation releases latched sprint";
        input.down(1, GameTouchInput.Action.SHIFT_TOGGLE, 0, 0, 2280);
        input.up(1, 2290);
        input.setGrabbed(false);
        assert !input.shiftToggled() : "Unlocking the pointer releases latched sprint";

        input.setGrabbed(true);
        input.setSensitivity(.5f);
        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 10, 20, 2300);
        input.move(1, 50, 30, 2310);
        input.up(1, 2320);
        assert e.values.equals(List.of("relative:20.0:5.0")) : "Sensitivity scales camera movement only: " + e.values;
        input.setSensitivity(Float.NaN);
        e.values.clear();
        input.down(1, GameTouchInput.Action.LOOK, 10, 20, 2330);
        input.move(1, 50, 30, 2340);
        input.up(1, 2350);
        assert e.values.equals(List.of("relative:40.0:10.0")) : "Invalid saved sensitivity safely uses default";
        System.out.println("GameTouchInput: multitouch, left tap, Ctrl/Space/RMB, Shift toggle/release, sensitivity, scroll and version floor passed");
    }
}
