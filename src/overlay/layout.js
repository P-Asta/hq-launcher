export function normalizePosition(position, fallback = { x: 50, y: 50 }) {
  const x = Number(position?.x);
  const y = Number(position?.y);
  return {
    x: Number.isFinite(x) ? Math.max(0, Math.min(100, x)) : fallback.x,
    y: Number.isFinite(y) ? Math.max(0, Math.min(100, y)) : fallback.y,
  };
}

export function normalizeWidgetPosition(position, fallback = { x: 50, y: 50, snap: true }) {
  return {
    ...normalizePosition(position, fallback),
    snap: position?.snap ?? fallback.snap ?? true,
  };
}

export function rectsOverlap(startA, endA, startB, endB) {
  return startA < endB && endA > startB;
}

export function snapOverlayPosition(rawPosition, id, _widgets, enabled = true, metrics = null) {
  const thresholdPx = 10;
  const guides = [];
  if (!enabled) return { position: normalizePosition(rawPosition), guides };
  let next = normalizePosition(rawPosition);

  if (!metrics?.dragRect || !metrics?.viewportWidth || !metrics?.viewportHeight) {
    return { position: next, guides };
  }

  const { dragRect, otherRects = [], viewportWidth, viewportHeight } = metrics;
  const current = {
    left: (next.x / 100) * viewportWidth,
    top: (next.y / 100) * viewportHeight,
    width: dragRect.width,
    height: dragRect.height,
  };
  current.right = current.left + current.width;
  current.bottom = current.top + current.height;

  let bestX = null;
  let bestY = null;
  const considerX = (candidate) => {
    if (candidate.distance <= thresholdPx && (!bestX || candidate.distance < bestX.distance)) {
      bestX = candidate;
    }
  };
  const considerY = (candidate) => {
    if (candidate.distance <= thresholdPx && (!bestY || candidate.distance < bestY.distance)) {
      bestY = candidate;
    }
  };

  [
    { x: 0, guide: 0, distance: Math.abs(current.left) },
    { x: viewportWidth - current.width, guide: viewportWidth, distance: Math.abs(current.right - viewportWidth) },
  ].forEach(considerX);
  [
    { y: 0, guide: 0, distance: Math.abs(current.top) },
    { y: viewportHeight - current.height, guide: viewportHeight, distance: Math.abs(current.bottom - viewportHeight) },
  ].forEach(considerY);

  for (const other of otherRects) {
    if (!other || other.id === id) continue;

    const verticalOverlap = rectsOverlap(current.top, current.bottom, other.top, other.bottom);
    const horizontalOverlap = rectsOverlap(current.left, current.right, other.left, other.right);

    if (verticalOverlap) {
      const candidates = [
        { x: other.left, guide: other.left, distance: Math.abs(current.left - other.left) },
        { x: other.left - current.width, guide: other.left, distance: Math.abs(current.right - other.left) },
        { x: other.right - current.width, guide: other.right, distance: Math.abs(current.right - other.right) },
        { x: other.right, guide: other.right, distance: Math.abs(current.left - other.right) },
      ];
      candidates.forEach(considerX);
    }

    if (horizontalOverlap) {
      const candidates = [
        { y: other.top, guide: other.top, distance: Math.abs(current.top - other.top) },
        { y: other.top - current.height, guide: other.top, distance: Math.abs(current.bottom - other.top) },
        { y: other.bottom - current.height, guide: other.bottom, distance: Math.abs(current.bottom - other.bottom) },
        { y: other.bottom, guide: other.bottom, distance: Math.abs(current.top - other.bottom) },
      ];
      candidates.forEach(considerY);
    }
  }

  if (bestX) {
    next = { ...next, x: (bestX.x / viewportWidth) * 100 };
    guides.push({ axis: "x", value: bestX.guide });
  }
  if (bestY) {
    next = { ...next, y: (bestY.y / viewportHeight) * 100 };
    guides.push({ axis: "y", value: bestY.guide });
  }

  return { position: normalizePosition(next), guides };
}

export function safeClassName(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export function safeOverlayId(value) {
  return safeClassName(value) || "overlay";
}

export function isInteractiveDragTarget(target) {
  return !!target?.closest?.("button,a,input,select,textarea,[role='button'],[data-no-drag='true']");
}

export function overlayRenderEntries(module, rendered) {
  const items = Array.isArray(rendered) ? rendered : [{ html: rendered }];
  return items
    .map((item, index) => {
      const entry = item && typeof item === "object" && !Array.isArray(item)
        ? item
        : { html: item };
      const childId = safeOverlayId(entry.id ?? index + 1);
      const widgetId = Array.isArray(rendered) ? `${module.id}:${childId}` : module.id;
      return {
        module,
        widgetId,
        renderId: widgetId,
        html: String(entry.html ?? ""),
        defaultPosition: normalizePosition(entry.defaultPosition, module.defaultPosition),
      };
    })
    .filter((entry) => entry.html);
}
