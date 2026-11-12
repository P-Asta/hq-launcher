package com.droiddeck.launcher.hq

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.os.Build
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import kotlin.math.hypot
import kotlin.math.max

/** Original HQ overlay; gesture behavior follows the inspected Amethyst input processors. */
@SuppressLint("ViewConstructor", "ClickableViewAccessibility")
class GameTouchControls(
    context: Context,
    version: Int,
    private val grabbed: () -> Boolean,
    onKey: (Int, Boolean) -> Unit,
    onRelative: (Float, Float) -> Unit,
    onAbsolute: (Float, Float) -> Unit,
    onMouse: (Int, Boolean) -> Unit,
    onWheel: (Int) -> Unit,
    private val onKeyboard: () -> Unit,
) : View(context) {
    private data class Button(val label: String, val action: GameTouchInput.Action?, val special: Int = 0, val rect: RectF = RectF()) {
        val id get() = action?.name ?: if (special == 1) "KEYBOARD" else "HQ"
    }
    private val sink = object : GameTouchInput.Sink {
        override fun key(code: Int, down: Boolean) = onKey(code, down)
        override fun relative(x: Float, y: Float) = onRelative(x, y)
        override fun absolute(x: Float, y: Float) = onAbsolute(x, y)
        override fun mouse(button: Int, down: Boolean) = onMouse(button, down)
        override fun wheel(steps: Int) = onWheel(steps)
    }
    private val settings = GameTouchSettings(context)
    private var defaultControls = emptyList<GameTouchSettings.Control>()
    private var gameVersion = version
    private var input = GameTouchInput(sink, version, dp(9f)).apply { setSensitivity(settings.sensitivity) }
    private var wasGrabbed = false
    private val fingers = mutableMapOf<Int, GameTouchInput.Action>()
    private val buttons = listOf(
        Button("E\nInteract", GameTouchInput.Action.INTERACT),
        Button("G\nDrop", GameTouchInput.Action.DROP),
        Button("Tab\nUtility", GameTouchInput.Action.UTILITY),
        Button("↑\nScroll", GameTouchInput.Action.SCROLL_UP),
        Button("↓\nScroll", GameTouchInput.Action.SCROLL_DOWN),
        Button("Esc", GameTouchInput.Action.ESCAPE),
        Button("Keys", null, 1), Button("HQ", null, 2),
        Button("Ctrl\nCrouch", GameTouchInput.Action.CONTROL),
        Button("Shift\nToggle", GameTouchInput.Action.SHIFT_TOGGLE),
        Button("Right\nClick", GameTouchInput.Action.RIGHT_CLICK),
        Button("Space\nJump", GameTouchInput.Action.SPACE),
    )
    private val drawOrder = buttons.filter { it.special != 2 } + buttons.filter { it.special == 2 }
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val line = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val text = Paint(Paint.ANTI_ALIAS_FLAG).apply { textAlign = Paint.Align.CENTER; typeface = android.graphics.Typeface.DEFAULT_BOLD }
    private var safeLeft = 0f; private var safeRight = 0f; private var safeTop = 0f; private var safeBottom = 0f
    private var joyX = 0f; private var joyY = 0f; private var radius = 0f
    private var knobX = 0f; private var knobY = 0f
    private var ready = false
    private val tick = object : Runnable {
        override fun run() {
            if (!isShown || !hasWindowFocus()) { releaseAll(); return }
            syncGrab()
            input.tick(SystemClock.uptimeMillis())
            invalidate()
            if (fingers.isNotEmpty()) postDelayed(this, 16)
        }
    }

    init {
        isFocusable = false
        contentDescription = "Lethal touch controls: joystick, E interact, G drop, scroll, Ctrl, Shift toggle, right click, Space, camera drag and left tap. HQ opens control settings."
        ready = true
    }
    private fun dp(value: Float) = value * resources.displayMetrics.density
    fun setGameVersion(version: Int) {
        if (gameVersion == version) return
        releaseAll(); gameVersion = version
        input = GameTouchInput(sink, version, dp(9f)).apply { setSensitivity(settings.sensitivity) }
        wasGrabbed = false; syncGrab(); layoutControls()
    }
    fun releaseAll() {
        removeCallbacks(tick); input.cancel(); fingers.clear(); knobX = 0f; knobY = 0f; invalidate()
    }
    private fun releaseTouches() {
        removeCallbacks(tick); input.cancelTouches(); fingers.clear(); knobX = 0f; knobY = 0f
    }
    private fun safeArea() = RectF(safeLeft, safeTop, width-safeRight, height-safeBottom)
    private fun showSettings() {
        releaseAll()
        settings.show(defaultControls, safeArea()) {
            input.setSensitivity(settings.sensitivity); layoutControls()
        }
    }
    private fun syncGrab() {
        val now = grabbed()
        if (now == wasGrabbed) return
        input.setGrabbed(now); wasGrabbed = now; fingers.clear(); knobX = 0f; knobY = 0f
        invalidate()
    }
    fun refreshPointerMode() = syncGrab()
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (ready && !hasFocus) releaseAll()
    }
    override fun onVisibilityChanged(changedView: View, visibility: Int) {
        super.onVisibilityChanged(changedView, visibility)
        if (ready && visibility != VISIBLE) releaseAll()
    }
    override fun onDetachedFromWindow() { settings.dismiss(); releaseAll(); super.onDetachedFromWindow() }
    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) { releaseAll(); layoutControls() }
    override fun onApplyWindowInsets(insets: WindowInsets): WindowInsets {
        val cutout = if (Build.VERSION.SDK_INT >= 28) insets.displayCutout else null
        safeLeft = (cutout?.safeInsetLeft ?: 0).toFloat(); safeRight = (cutout?.safeInsetRight ?: 0).toFloat()
        safeTop = (cutout?.safeInsetTop ?: 0).toFloat(); safeBottom = (cutout?.safeInsetBottom ?: 0).toFloat()
        releaseAll(); layoutControls(); return super.onApplyWindowInsets(insets)
    }
    private fun layoutControls() {
        if (width == 0 || height == 0) return
        val fit = ((height - safeTop - safeBottom) / dp(400f)).coerceIn(.7f, 1f)
        val size = max(dp(48f), dp(56f) * fit)
        val gap = dp(12f) * fit
        val right = width - safeRight - dp(20f) - size / 2
        val bottom = height - safeBottom - dp(20f) - size / 2
        fun put(index: Int, x: Float, y: Float) { buttons[index].rect.set(x-size/2, y-size/2, x+size/2, y+size/2) }
        put(0, right, bottom - size - gap); put(1, right, bottom)
        put(2, right, bottom - 2*(size+gap))
        put(3, right-size-gap, bottom-size-gap); put(4, right-size-gap, bottom)
        put(5, safeLeft+dp(20f)+size/2, safeTop+dp(12f)+size/2)
        put(6, safeLeft+dp(20f)+size*1.5f+gap, safeTop+dp(12f)+size/2)
        put(7, right, safeTop+dp(12f)+size/2)
        radius = dp(70f) * fit
        joyX = safeLeft + radius + dp(28f)
        joyY = height - safeBottom - radius - dp(28f)
        put(8, joyX+radius+gap+size/2, bottom)
        put(9, joyX, joyY-radius-gap-size/2)
        put(10, right-size-gap, bottom-2*(size+gap))
        put(11, right-2*(size+gap), bottom)
        val area = safeArea()
        defaultControls = buttons.filter { it.action != GameTouchInput.Action.UTILITY || input.utilityEnabled() }.map {
            GameTouchSettings.Control(it.id, it.label, RectF(it.rect))
        } + GameTouchSettings.Control("JOYSTICK", "Move", RectF(joyX-radius, joyY-radius, joyX+radius, joyY+radius), true)
        for (control in defaultControls) {
            val box = settings.bounds(control, area)
            if (control.circle) { joyX = box.centerX(); joyY = box.centerY(); radius = box.width()/2 }
            else buttons.first { it.id == control.id }.rect.set(box)
        }
        invalidate()
    }
    private fun shown(button: Button) = (wasGrabbed || button.special != 0 || button.action == GameTouchInput.Action.ESCAPE) &&
        (button.action != GameTouchInput.Action.UTILITY || input.utilityEnabled())
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        fill.color = Color.argb(38, 0, 0, 0); line.color = Color.argb(125, 255, 255, 255); line.strokeWidth = dp(1.2f)
        if (wasGrabbed) {
            canvas.drawCircle(joyX, joyY, radius, fill); canvas.drawCircle(joyX, joyY, radius, line)
            fill.color = Color.argb(75, 255, 255, 255)
            canvas.drawCircle(joyX + knobX, joyY + knobY, radius*.36f, fill)
        }
        for (button in drawOrder) {
            if (!shown(button)) continue
            val latched = button.action == GameTouchInput.Action.SHIFT_TOGGLE && input.shiftToggled()
            val held = latched || button.action != null && fingers.values.any { it == button.action }
            fill.color = if (held) Color.argb(115, 255, 255, 255) else Color.argb(58, 20, 20, 20)
            line.color = Color.argb(if (held) 230 else 135, 255, 255, 255)
            canvas.drawRoundRect(button.rect, dp(6f), dp(6f), fill)
            canvas.drawRoundRect(button.rect, dp(6f), dp(6f), line)
            val parts = button.label.split('\n')
            text.color = Color.argb(230, 255, 255, 255)
            text.textSize = dp(if (parts.size == 1 || parts[0].length > 2) 14f else 21f)
            val labelWidth = text.measureText(parts[0])
            if (labelWidth > button.rect.width()-dp(8f)) text.textSize *= (button.rect.width()-dp(8f))/labelWidth
            val y = button.rect.centerY() - (text.ascent()+text.descent())/2 - if (parts.size > 1) dp(7f) else 0f
            canvas.drawText(parts[0], button.rect.centerX(), y, text)
            if (parts.size > 1) { text.textSize = dp(10f); canvas.drawText(if (latched) "ON" else parts[1], button.rect.centerX(), button.rect.bottom-dp(8f), text) }
        }
    }
    private fun move(id: Int, action: GameTouchInput.Action, x: Float, y: Float, time: Long) {
        if (action == GameTouchInput.Action.JOYSTICK) {
            val dx = x-joyX; val dy = y-joyY; val length = max(radius, hypot(dx, dy))
            knobX = dx/length*radius; knobY = dy/length*radius
            input.move(id, dx/radius, dy/radius, time)
        } else input.move(id, x, y, time)
    }
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.isFromSource(InputDevice.SOURCE_MOUSE)) return false
        syncGrab()
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                if (event.actionMasked == MotionEvent.ACTION_DOWN) releaseTouches()
                val i=event.actionIndex; val id=event.getPointerId(i); val x=event.getX(i); val y=event.getY(i)
                val button=drawOrder.asReversed().firstOrNull { shown(it) && it.rect.contains(x,y) }
                if (button != null && button.special in 1..2) {
                    releaseAll(); if (button.special == 1) onKeyboard() else showSettings(); return true
                }
                val action = button?.action ?: if (wasGrabbed && hypot(x-joyX,y-joyY) <= radius*1.2f) GameTouchInput.Action.JOYSTICK else GameTouchInput.Action.LOOK
                if (action == GameTouchInput.Action.JOYSTICK && fingers.values.any { it == action }) return true
                fingers[id]=action
                input.down(id, action, if (action == GameTouchInput.Action.JOYSTICK) 0f else x, if (action == GameTouchInput.Action.JOYSTICK) 0f else y, event.eventTime)
                if (action == GameTouchInput.Action.JOYSTICK) move(id, action, x, y, event.eventTime)
                removeCallbacks(tick); postDelayed(tick,16)
            }
            MotionEvent.ACTION_MOVE -> for ((id, action) in fingers.toMap()) {
                val i=event.findPointerIndex(id)
                if (i<0) { releaseAll(); break }
                move(id, action, event.getX(i), event.getY(i), event.eventTime)
            }
            MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_UP -> {
                val id=event.getPointerId(event.actionIndex)
                if (fingers[id] == GameTouchInput.Action.LOOK) {
                    move(id, GameTouchInput.Action.LOOK, event.getX(event.actionIndex), event.getY(event.actionIndex), event.eventTime)
                }
                if (fingers.remove(id) == GameTouchInput.Action.JOYSTICK) { knobX=0f; knobY=0f }
                input.up(id,event.eventTime)
                if (fingers.isEmpty()) removeCallbacks(tick)
            }
            MotionEvent.ACTION_CANCEL -> releaseAll()
        }
        invalidate(); return true
    }
}
