export function extractSpreadsheetId(value) {
  const text = String(value ?? "").trim();
  const match = text.match(/\/spreadsheets\/d\/([^/?#]+)/);
  return match?.[1] ?? text;
}

export function decodeUrlPart(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function parseSpreadsheetInput(value) {
  const text = String(value ?? "").trim();
  const spreadsheetMatch = text.match(/\/spreadsheets\/d\/([^/?#]+)/);
  const gidMatch = text.match(/[?&#]gid=([^&#]+)/);
  return {
    spreadsheetId: spreadsheetMatch?.[1] ?? text,
    sheetGid: gidMatch ? decodeUrlPart(gidMatch[1]) : "",
    hasSpreadsheetUrl: !!spreadsheetMatch,
  };
}

export function normalizeSheetInfo(info) {
  if (typeof info === "string") {
    return info ? { sheetId: "", title: info } : null;
  }
  const title = info?.title ?? info?.name;
  if (title == null || String(title) === "") return null;
  const sheetId = info?.sheetId ?? info?.sheet_id ?? info?.id ?? "";
  return {
    sheetId: sheetId == null ? "" : String(sheetId),
    title: String(title),
  };
}

export function normalizeSheetInfos(value) {
  if (!Array.isArray(value)) return [];
  return value.map(normalizeSheetInfo).filter(Boolean);
}

export function findSheetTitleByGid(sheetInfos, sheetGid) {
  const gid = String(sheetGid ?? "").trim();
  if (!gid) return "";
  return sheetInfos.find((sheet) => sheet.sheetId === gid)?.title ?? "";
}

export function findSheetInfoByTitle(sheetInfos, title) {
  const text = String(title ?? "");
  if (!text) return null;
  return sheetInfos.find((sheet) => sheet.title === text) ?? null;
}

export function normalizeSheetMatchName(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

export function findSimilarSheetInfoByTitle(sheetInfos, title) {
  const preferred = normalizeSheetMatchName(title);
  if (!preferred) return null;
  return (
    sheetInfos.find((sheet) => normalizeSheetMatchName(sheet.title) === preferred) ??
    sheetInfos.find((sheet) => {
      const candidate = normalizeSheetMatchName(sheet.title);
      return candidate.includes(preferred) || preferred.includes(candidate);
    }) ??
    null
  );
}

export function normalizeSheetColumn(value, fallback) {
  const text = String(value ?? "")
    .trim()
    .toUpperCase()
    .replace(/[^A-Z]/g, "");
  return text || fallback;
}

export function normalizeSheetColumnList(value, fallback) {
  const text = String(value ?? "")
    .trim()
    .toUpperCase()
    .replace(/[^A-Z,]/g, "")
    .replace(/,{2,}/g, ",");
  return text || fallback;
}
