export function fmtBytes(n) {
  if (typeof n !== "number" || !Number.isFinite(n)) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toFixed(digits)} ${units[i]}`;
}

export function formatTransferProgress(task) {
  const downloaded = Number(task?.downloaded_bytes);
  if (!Number.isFinite(downloaded)) return "";
  const total = Number(task?.total_bytes);
  if (Number.isFinite(total) && total > 0) {
    return `Download ${fmtBytes(downloaded)} / ${fmtBytes(total)}`;
  }
  return downloaded > 0 ? `Download ${fmtBytes(downloaded)}` : "";
}

export function formatExtractProgress(task) {
  const done = Number(task?.extracted_files);
  const total = Number(task?.total_files);
  if (Number.isFinite(done) && Number.isFinite(total) && total > 0) {
    return `Extract ${done.toLocaleString()} / ${total.toLocaleString()} files`;
  }
  return "";
}

export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

export function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function toOptionalNumber(value) {
  if (value == null || value === "") return null;
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

export function valueLabel(v) {
  if (!v) return "";
  if (v.type === "Bool") return v.data ? "true" : "false";
  if (v.type === "String") return v.data ?? "";
  if (v.type === "Int") return String(v.data?.value ?? "");
  if (v.type === "Float") return String(v.data?.value ?? "");
  if (v.type === "Enum") return v.data?.options?.[v.data?.index ?? 0] ?? "";
  if (v.type === "Flags")
    return (v.data?.indicies ?? [])
      .map((i) => v.data?.options?.[i])
      .filter(Boolean)
      .join(", ");
  return "";
}
