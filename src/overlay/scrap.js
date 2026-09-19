import { arrayValue, firstValue, intish, pickMatchingKey } from "./values";

export function cleanOverlayName(value, fallback = "Unknown") {
  const text = String(value ?? "")
    .replace(/^"+|"+$/g, "")
    .replace(/\(Clone\)$/i, "")
    .trim();
  return text || fallback;
}

export function looksLikeOverlayName(value) {
  if (typeof value !== "string") return false;
  const text = cleanOverlayName(value);
  if (!text || text === "Unknown") return false;
  if (/^\d+$/.test(text)) return false;
  if (/^(true|false|null|undefined|none)$/i.test(text)) return false;
  if (/^(ship|level|moon|inside|outside|main|entrance|fire|exit)$/i.test(text)) return false;
  return /[a-z]/i.test(text);
}

export function findNestedOverlayName(root) {
  const preferredKeys = /name|item|scrap|object|prop|display/i;
  const ignoredKeys = /location|position|rotation|scale|spawn|collected|previous|day|value|price|weight|id|seed/i;
  const seen = new Set();
  const stack = [{ value: root, score: 0 }];
  let best = "";
  let bestScore = -1;

  while (stack.length > 0) {
    const entry = stack.pop();
    const value = entry?.value;
    const score = Number(entry?.score ?? 0);

    if (typeof value === "string") {
      if (looksLikeOverlayName(value) && score > bestScore) {
        best = cleanOverlayName(value);
        bestScore = score;
      }
      continue;
    }

    if (!value || typeof value !== "object" || seen.has(value)) continue;
    seen.add(value);

    for (const [key, child] of Object.entries(value)) {
      if (ignoredKeys.test(key)) continue;
      stack.push({ value: child, score: preferredKeys.test(key) ? score + 3 : score });
    }
  }

  return best;
}

export function normalizeScrapItem(item) {
  const name = findNestedOverlayName(item) || "Unknown";
  const value = intish(pickMatchingKey(item, /^(scrap)?value$|^price$/i));
  return { name, value, raw: item };
}

export function overlayDataSources(context) {
  return [
    context?.endSummary?.payload,
    context?.lcstats,
    context?.endSummary,
  ].filter(Boolean);
}

export function collectScrapItemsFromSource(source) {
  const direct = pickMatchingKey(source, /^(missed|uncollected|left|remaining).*items$|^scrapMissed$/i);
  return arrayValue(direct)
    .filter((item) => item && typeof item === "object")
    .map(normalizeScrapItem)
    .filter((item) => item.name !== "Unknown" || item.value > 0)
    .sort((a, b) => b.value - a.value);
}

export function uniqueScrapItems(items) {
  const unique = [];
  const seen = new Set();
  for (const item of arrayValue(items)) {
    const key = `${item.name}:${item.value}`;
    if (seen.has(key)) continue;
    seen.add(key);
    unique.push(item);
  }
  return unique;
}

export function groupScrapItems(items) {
  const groups = new Map();
  for (const item of arrayValue(items)) {
    const name = item.name || "Unknown";
    const current = groups.get(name) ?? { name, values: [], total: 0, max: 0, count: 0, items: [] };
    current.values.push(item.value);
    current.items.push(item);
    current.total += item.value;
    current.max = Math.max(current.max, item.value);
    current.count += 1;
    groups.set(name, current);
  }

  return [...groups.values()]
    .map((group) => ({ ...group, values: group.values.sort((a, b) => b - a) }))
    .sort((a, b) => b.max - a.max || b.total - a.total || a.name.localeCompare(b.name));
}

export function remainingScrapFromTotals(source) {
  const perf = pickMatchingKey(source, /^performanceInfo$/i) ?? {};
  const explicit = firstValue(
    pickMatchingKey(source, /^(remaining|missed|uncollected).*scrap$/i),
    pickMatchingKey(perf, /^(remaining|missed|uncollected).*scrap$/i),
  );
  if (explicit !== undefined) return intish(explicit);

  const available = firstValue(
    pickMatchingKey(perf, /^totalAvailableValue$/i),
    pickMatchingKey(source, /^totalAvailableValue$|^bottomLineTrue$/i),
  );
  const collected = firstValue(
    pickMatchingKey(perf, /^collectedTotal$/i),
    pickMatchingKey(source, /^collectedTotal$/i),
  );
  if (available !== undefined && collected !== undefined) {
    return Math.max(0, intish(available) - intish(collected));
  }
  return null;
}

export function remainingScrapFromEndText(summary) {
  const text = [summary?.title, ...arrayValue(summary?.lines)].join("\n");
  const match = text.match(/(?:scrap|left|missed|remaining|uncollected)[^\d-]*(\d[\d,]*)/i);
  return match ? intish(match[1].replace(/,/g, "")) : null;
}

export function normalizedScrapSummary(context) {
  const sources = overlayDataSources(context);
  const items = uniqueScrapItems(sources.flatMap(collectScrapItemsFromSource));
  const itemTotal = items.reduce((total, item) => total + item.value, 0);
  const source = sources[0] ?? null;
  const total = itemTotal || (
    sources.map(remainingScrapFromTotals).find((value) => value !== null)
    ?? remainingScrapFromEndText(context?.endSummary)
    ?? null
  );
  const moon = cleanOverlayName(firstValue(
    pickMatchingKey(pickMatchingKey(source, /^moonInfo$/i) ?? {}, /^name$/i),
    pickMatchingKey(source, /^(moonName|moon|levelName)$/i),
    context?.endSummary?.title,
  ));

  return {
    moon,
    total,
    remaining: total,
    items,
    groups: groupScrapItems(items),
  };
}
