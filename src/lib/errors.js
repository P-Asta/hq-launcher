export function isAuthError(e) {
  const msg = e?.message ?? String(e ?? "");
  const m = msg.toLowerCase();
  return (
    m.includes("not logged in") ||
    m.includes("two-factor") ||
    m.includes("steam guard") ||
    m.includes("missing username for remembered login")
  );
}
