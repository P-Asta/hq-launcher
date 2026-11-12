package com.droiddeck.launcher.hq

import android.annotation.SuppressLint
import android.app.Dialog
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowInsets
import android.view.WindowManager
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import org.json.JSONObject
import java.util.Locale
import kotlin.math.roundToInt

/** HQ-only settings; native modal windows never send editing gestures to the guest. */
internal class GameTouchSettings(private val context: Context) {
    data class Control(val id: String, val label: String, val defaults: RectF, val circle: Boolean = false)
    private data class Placement(val x: Float, val y: Float, val scale: Float)
    private val prefs = context.getSharedPreferences("hq-touch-controls", Context.MODE_PRIVATE)
    private var activeDialog: Dialog? = null
    private var layout = readLayout()
    var sensitivity: Float = runCatching { prefs.getFloat("sensitivity", 1f) }.getOrDefault(1f)
        .let { if (it.isFinite()) it.coerceIn(.25f, 3f) else 1f }
        private set
    private val ink = Color.rgb(234, 242, 237)
    private val accent = Color.rgb(78, 230, 182)
    private fun dp(value: Float) = value * context.resources.displayMetrics.density
    private fun readLayout(): Map<String, Placement> = runCatching {
        val json = JSONObject(prefs.getString("layout-v1", "{}") ?: "{}")
        json.keys().asSequence().take(64).mapNotNull { id ->
            val item = json.optJSONObject(id) ?: return@mapNotNull null
            val x = item.optDouble("x").toFloat(); val y = item.optDouble("y").toFloat()
            if (!x.isFinite() || !y.isFinite()) return@mapNotNull null
            id to Placement(x.coerceIn(0f, 1f), y.coerceIn(0f, 1f), TouchControlGeometry.scale(item.optDouble("scale", 1.0).toFloat()))
        }.toMap()
    }.getOrDefault(emptyMap())
    private fun defaultPlacement(control: Control, area: RectF) = Placement(
        TouchControlGeometry.normalized(control.defaults.centerX(), area.left, area.right),
        TouchControlGeometry.normalized(control.defaults.centerY(), area.top, area.bottom), 1f)
    private fun resolve(control: Control, placement: Placement, area: RectF): RectF {
        val edge = if (control.circle) control.defaults.width() * placement.scale
            else (control.defaults.width() * placement.scale).coerceAtLeast(dp(44f))
        val b = TouchControlGeometry.bounds(area.left, area.top, area.right, area.bottom, placement.x, placement.y, edge)
        return RectF(b[0], b[1], b[2], b[3])
    }
    fun bounds(control: Control, area: RectF) = resolve(control, layout[control.id] ?: defaultPlacement(control, area), area)
    fun dismiss() { activeDialog?.dismiss(); activeDialog = null }
    private fun panel() = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL; setPadding(dp(20f).toInt(), dp(16f).toInt(), dp(20f).toInt(), dp(16f).toInt())
        background = GradientDrawable().apply { setColor(Color.rgb(16, 23, 19)); cornerRadius = dp(16f) }
    }
    private fun label(value: String, size: Float = 16f) = TextView(context).apply {
        text = value; textSize = size; setTextColor(ink); setPadding(0, dp(6f).toInt(), 0, dp(6f).toInt())
    }
    private fun button(value: String, action: () -> Unit) = Button(context).apply {
        text = value; isAllCaps = false; setTextColor(ink); minHeight = dp(48f).toInt()
        backgroundTintList = ColorStateList.valueOf(Color.rgb(34, 49, 40)); setOnClickListener { action() }
    }
    private fun showDialog(content: View, full: Boolean = false): Dialog {
        dismiss()
        val dialog = Dialog(context).apply { requestWindowFeature(Window.FEATURE_NO_TITLE); setContentView(content) }
        activeDialog = dialog
        dialog.setOnDismissListener { if (activeDialog === dialog) activeDialog = null }
        dialog.show()
        dialog.window?.apply {
            setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            if (full) {
                clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
                addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN)
                if (Build.VERSION.SDK_INT >= 28) attributes = attributes.apply {
                    layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
                }
                decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_FULLSCREEN or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or View.SYSTEM_UI_FLAG_LAYOUT_STABLE or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
            }
            setLayout(if (full) ViewGroup.LayoutParams.MATCH_PARENT else minOf(dp(360f).toInt(), context.resources.displayMetrics.widthPixels-dp(32f).toInt()),
                if (full) ViewGroup.LayoutParams.MATCH_PARENT else ViewGroup.LayoutParams.WRAP_CONTENT)
        }
        return dialog
    }
    fun show(controls: List<Control>, area: RectF, onChanged: () -> Unit) {
        if (activeDialog != null) return
        val content = panel()
        content.addView(label("HQ Controls", 20f).apply { setTypeface(typeface, Typeface.BOLD) })
        content.addView(button("Mouse sensitivity") { showSensitivity(onChanged) })
        content.addView(button("Key layout") { showLayout(controls, area, onChanged) })
        content.addView(button("Done") { dismiss() })
        showDialog(content)
    }
    private fun showSensitivity(onChanged: () -> Unit) {
        val content = panel()
        content.addView(label("Mouse sensitivity", 20f))
        val value = label(String.format(Locale.US, "%.2f×", sensitivity))
        content.addView(value)
        val slider = SeekBar(context).apply {
            max = 55; progress = ((sensitivity-.25f)/.05f).roundToInt()
            contentDescription = "Mouse sensitivity"; progressTintList = ColorStateList.valueOf(accent)
            thumbTintList = ColorStateList.valueOf(accent)
        }
        slider.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar, progress: Int, fromUser: Boolean) {
                sensitivity = .25f + progress * .05f
                prefs.edit().putFloat("sensitivity", sensitivity).apply()
                value.text = String.format(Locale.US, "%.2f×", sensitivity); onChanged()
            }
            override fun onStartTrackingTouch(seekBar: SeekBar) {}
            override fun onStopTrackingTouch(seekBar: SeekBar) {}
        })
        content.addView(slider, LinearLayout.LayoutParams(-1, dp(48f).toInt()))
        content.addView(button("Reset sensitivity") { slider.progress = 15 })
        content.addView(button("Done") { dismiss() })
        showDialog(content)
    }
    private fun showLayout(controls: List<Control>, sourceArea: RectF, onChanged: () -> Unit) {
        if (controls.isEmpty()) return
        val draft = layout.toMutableMap()
        val root = FrameLayout(context)
        val heading = label("Key layout", 18f).apply { setTypeface(typeface, Typeface.BOLD) }
        val selected = label("", 13f)
        val slider = SeekBar(context).apply {
            max = 20; contentDescription = "Control size"; thumbTintList = ColorStateList.valueOf(accent)
            progressTintList = ColorStateList.valueOf(accent)
        }
        lateinit var editor: LayoutEditor
        val updateSelection: (Control) -> Unit = { control ->
            val scale = draft[control.id]?.scale ?: 1f
            selected.text = "${control.label.replace('\n', ' ')} · ${(scale*100).roundToInt()}%"
            slider.progress = ((scale-.75f)/.05f).roundToInt()
        }
        editor = LayoutEditor(controls, sourceArea, draft, updateSelection)
        root.addView(editor, FrameLayout.LayoutParams(-1, -1))
        val tools = panel().apply { setPadding(dp(12f).toInt(), dp(6f).toInt(), dp(12f).toInt(), dp(8f).toInt()) }
        val actions = LinearLayout(context).apply { gravity = Gravity.CENTER_VERTICAL }
        actions.addView(heading, LinearLayout.LayoutParams(0, -2, 1f))
        actions.addView(button("↕") {
            val position = tools.layoutParams as FrameLayout.LayoutParams
            val atTop = position.gravity and Gravity.VERTICAL_GRAVITY_MASK == Gravity.TOP
            position.gravity = (if (atTop) Gravity.BOTTOM else Gravity.TOP) or Gravity.CENTER_HORIZONTAL
            position.topMargin = if (atTop) 0 else dp(8f).toInt()
            position.bottomMargin = if (atTop) dp(8f).toInt() else 0
            tools.layoutParams = position
        }.apply {
            contentDescription = "Move tools to the other edge"
            minWidth = dp(44f).toInt(); minimumWidth = dp(44f).toInt()
        }, LinearLayout.LayoutParams(dp(44f).toInt(), -2))
        actions.addView(button("Reset") { draft.clear(); editor.invalidate(); updateSelection(editor.selected) })
        actions.addView(button("Done") {
            val json = JSONObject()
            for ((id, item) in draft) json.put(id, JSONObject().put("x", item.x).put("y", item.y).put("scale", item.scale))
            layout = draft.toMap(); prefs.edit().putString("layout-v1", json.toString()).apply()
            onChanged(); dismiss()
        })
        tools.addView(actions)
        tools.addView(label("Drag a control. Select one to resize.", 12f))
        tools.addView(selected)
        tools.addView(slider, LinearLayout.LayoutParams(-1, dp(40f).toInt()))
        slider.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar, progress: Int, fromUser: Boolean) {
                if (!fromUser) return
                val control = editor.selected
                val previous = draft[control.id] ?: defaultPlacement(control, sourceArea)
                draft[control.id] = previous.copy(scale = .75f + progress * .05f)
                selected.text = "${control.label.replace('\n', ' ')} · ${((.75f+progress*.05f)*100).roundToInt()}%"
                editor.invalidate()
            }
            override fun onStartTrackingTouch(seekBar: SeekBar) {}
            override fun onStopTrackingTouch(seekBar: SeekBar) {}
        })
        root.addView(tools, FrameLayout.LayoutParams(minOf(dp(360f).toInt(), context.resources.displayMetrics.widthPixels-dp(24f).toInt()), -2, Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply { topMargin = dp(8f).toInt() })
        updateSelection(editor.selected)
        showDialog(root, full = true)
    }
    @SuppressLint("ViewConstructor", "ClickableViewAccessibility")
    private inner class LayoutEditor(
        val controls: List<Control>, val source: RectF, val draft: MutableMap<String, Placement>, val onSelected: (Control) -> Unit,
    ) : View(context) {
        var selected = controls.first(); private set
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private var safe = RectF()
        private var insetLeft = 0; private var insetRight = 0; private var insetTop = 0; private var insetBottom = 0
        private var pointer = -1; private var offsetX = 0f; private var offsetY = 0f
        private val drawOrder = controls.sortedBy { it.id == "HQ" }
        init { contentDescription = "Key layout editor. Drag controls to move them, then select Control size to resize." }
        override fun onApplyWindowInsets(insets: WindowInsets): WindowInsets {
            val cutout = if (Build.VERSION.SDK_INT >= 28) insets.displayCutout else null
            insetLeft = cutout?.safeInsetLeft ?: 0; insetRight = cutout?.safeInsetRight ?: 0
            insetTop = cutout?.safeInsetTop ?: 0; insetBottom = cutout?.safeInsetBottom ?: 0
            updateArea(); return super.onApplyWindowInsets(insets)
        }
        override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) { updateArea() }
        private fun updateArea() { safe.set(insetLeft.toFloat(), insetTop.toFloat(), (width-insetRight).toFloat(), (height-insetBottom).toFloat()); invalidate() }
        private fun rect(control: Control) = resolve(control, draft[control.id] ?: defaultPlacement(control, source), safe)
        override fun onDraw(canvas: Canvas) {
            canvas.drawColor(Color.argb(180, 0, 0, 0))
            for (control in drawOrder) {
                val box = rect(control)
                paint.style = Paint.Style.FILL; paint.color = Color.argb(140, 20, 30, 23)
                if (control.circle) canvas.drawOval(box, paint) else canvas.drawRoundRect(box, dp(6f), dp(6f), paint)
                paint.style = Paint.Style.STROKE; paint.strokeWidth = dp(if (selected.id == control.id) 2f else 1f)
                paint.color = if (selected.id == control.id) accent else ink
                if (control.circle) canvas.drawOval(box, paint) else canvas.drawRoundRect(box, dp(6f), dp(6f), paint)
                paint.style = Paint.Style.FILL; paint.textAlign = Paint.Align.CENTER; paint.typeface = Typeface.DEFAULT_BOLD
                paint.textSize = minOf(dp(13f), box.width()/5f)
                val lines = control.label.split('\n'); val lineHeight = paint.textSize*1.2f
                val firstY = box.centerY()-(paint.ascent()+paint.descent())/2-(lines.size-1)*lineHeight/2
                lines.forEachIndexed { i, line -> canvas.drawText(line, box.centerX(), firstY+i*lineHeight, paint) }
            }
        }
        override fun onTouchEvent(event: MotionEvent): Boolean {
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    val hit = drawOrder.asReversed().firstOrNull { rect(it).contains(event.x, event.y) } ?: return true
                    selected = hit; pointer = event.getPointerId(0)
                    val box = rect(hit); offsetX = event.x-box.centerX(); offsetY = event.y-box.centerY()
                    onSelected(hit); invalidate()
                }
                MotionEvent.ACTION_MOVE -> {
                    val i = event.findPointerIndex(pointer)
                    if (i < 0) return true
                    val old = draft[selected.id] ?: defaultPlacement(selected, source)
                    draft[selected.id] = old.copy(x = TouchControlGeometry.normalized(event.getX(i)-offsetX, safe.left, safe.right),
                        y = TouchControlGeometry.normalized(event.getY(i)-offsetY, safe.top, safe.bottom))
                    invalidate()
                }
                MotionEvent.ACTION_POINTER_UP -> if (event.getPointerId(event.actionIndex) == pointer) pointer = -1
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> { pointer = -1; performClick() }
            }
            return true
        }
    }
}
