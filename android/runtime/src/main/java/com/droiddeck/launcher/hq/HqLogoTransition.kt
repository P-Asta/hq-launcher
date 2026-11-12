package com.droiddeck.launcher.hq

import android.animation.ValueAnimator
import android.graphics.Bitmap
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.caverock.androidsvg.SVG
import com.droiddeck.launcher.ui.Motion

/** Uses the exact public/icon.svg shipped by the launcher, copied by Gradle. */
@Composable
fun HqLogoTransition(modifier: Modifier = Modifier, running: Boolean = true, percent: Int = -1) {
    val context = LocalContext.current
    val logo = remember(context) {
        val bitmap = Bitmap.createBitmap(384, 384, Bitmap.Config.ARGB_8888)
        context.assets.open("hq/icon.svg").use {
            SVG.getFromInputStream(it).apply {
                documentWidth = 384f
                documentHeight = 384f
            }.renderToCanvas(android.graphics.Canvas(bitmap))
        }
        bitmap.asImageBitmap()
    }
    val moving = running && Motion.scale > 0f && ValueAnimator.areAnimatorsEnabled()
    val reveal = when {
        percent >= 0 -> percent.coerceIn(0, 100) / 100f
        !moving -> 1f
        else -> rememberInfiniteTransition(label = "hq-logo")
            .animateFloat(0f, 1.25f, infiniteRepeatable(tween(1600, easing = FastOutSlowInEasing)), label = "hq-reveal").value
    }
    Canvas(modifier.aspectRatio(1f)) {
        val bounds = IntSize(size.width.toInt(), size.height.toInt())
        drawImage(logo, dstSize = bounds, alpha = 0.22f)
        clipRect(top = size.height * (1f - reveal.coerceAtMost(1f))) {
            val alpha = if (reveal > 1.1f) ((1.25f - reveal) / 0.15f).coerceIn(0f, 1f) else 1f
            drawImage(logo, dstSize = bounds, alpha = alpha)
        }
    }
}

/** Covers the outgoing frame before running the existing session close action. */
@Composable
fun HqExitTransition(label: String, onCovered: () -> Unit) {
    val opacity = remember { Animatable(0f) }
    val covered = rememberUpdatedState(onCovered)
    LaunchedEffect(Unit) {
        if (ValueAnimator.areAnimatorsEnabled() && Motion.scale > 0f) {
            opacity.animateTo(1f, Motion.tw(160))
        } else {
            opacity.snapTo(1f)
            withFrameNanos { }
        }
        covered.value()
    }
    Column(
        modifier = Modifier.fillMaxSize().graphicsLayer { alpha = opacity.value }
            .background(Color(0xFF090E0B))
            .pointerInput(Unit) { awaitEachGesture { while (true) awaitPointerEvent().changes.forEach { it.consume() } } },
        verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        HqLogoTransition(Modifier.width(96.dp), running = false)
        Text(label, color = Color.White)
    }
}
