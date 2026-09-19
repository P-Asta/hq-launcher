import { intish, normalizeVersion, stripLcQuote, valueAt, valueAtAny } from "./values";

export const HQ_FIRESTORE_PROJECT = "highquotahq214";

export const HQ_FIRESTORE_API_KEY = "AIzaSyCklz28QDpVHdagTruIxlPc5hdi-fj6QxE";

export const hqRunsSessionCache = new Map();

export const HQ_COLLECTIONS = {
  hq: "leaderboards_hq",
  sdc: "leaderboards_sdc",
  smhq: "leaderboards_smhq",
};

export const HQ_LEADERBOARD_COLLECTIONS = {
  vanilla: {
    hq: "leaderboards_hq",
    sdc: "leaderboards_sdc",
    smhq: "leaderboards_smhq",
  },
  modded: {
    hq: "modded_hq",
    sdc: "modded_sdc",
    smhq: "modded_smhq",
  },
  legacyModded: {
    brutal: {
      hq: "lc_modded_brutal_hq",
      sdc: "lc_modded_brutal_sdc",
      smhq: "lc_modded_brutal_smhq",
    },
    eclipsed: {
      hq: "lc_modded_eclipsed_hq",
      smhq: "lc_modded_eclipsed_smhq",
    },
    wesleysMoons: {
      hq: "lc_modded_wesleysmoons_hq",
      sdc: "lc_modded_wesleysmoons_sdc",
      smhq: "lc_modded_wesleysmoons_smhq",
    },
    classicMoons: {
      hq: "lc_modded_classicmoons_hq",
      sdc: "lc_modded_classicmoons_sdc",
      smhq: "lc_modded_classicmoons_smhq",
    },
  },
};

export const HQ_LEADERBOARD_BOARD_TYPES = {
  hq: { id: "hq", name: "Classic High Quota", metricLabel: "Quota Amount", metricKey: "quotaAmount" },
  sdc: { id: "sdc", name: "Single Day Clear", metricLabel: "Total Scrap", metricKey: "totalScrap" },
  smhq: { id: "smhq", name: "Single Moon High Quota", metricLabel: "Quota Amount", metricKey: "quotaAmount" },
};

export const HQ_LEADERBOARD_RUN_FIELDS = [
  "id",
  "collectionName",
  "players",
  "version",
  "verified",
  "quotaAmount",
  "quotaReached",
  "totalScrap",
  "moon",
  "scrapType",
  "videos",
  "date",
  "verifiedAt",
  "verifier",
];

export function firestoreValue(value) {
  if (!value || typeof value !== "object") return null;
  if ("stringValue" in value) return value.stringValue;
  if ("integerValue" in value) return Number(value.integerValue);
  if ("doubleValue" in value) return Number(value.doubleValue);
  if ("booleanValue" in value) return !!value.booleanValue;
  if ("timestampValue" in value) return value.timestampValue;
  if ("arrayValue" in value) return (value.arrayValue.values ?? []).map(firestoreValue);
  if ("mapValue" in value) {
    return Object.fromEntries(
      Object.entries(value.mapValue.fields ?? {}).map(([key, item]) => [key, firestoreValue(item)]),
    );
  }
  return null;
}

export function firestoreDocument(doc) {
  const data = Object.fromEntries(
    Object.entries(doc?.fields ?? {}).map(([key, value]) => [key, firestoreValue(value)]),
  );
  return { id: String(doc?.name ?? "").split("/").pop(), ...data };
}

export async function fetchHqRuns(collectionName) {
  if (hqRunsSessionCache.has(collectionName)) return hqRunsSessionCache.get(collectionName);

  const url = `https://firestore.googleapis.com/v1/projects/${HQ_FIRESTORE_PROJECT}/databases/(default)/documents/${collectionName}?pageSize=1000&key=${HQ_FIRESTORE_API_KEY}`;
  const request = fetch(url)
    .then((response) => {
      if (!response.ok) throw new Error(`HighQuotaHQ fetch failed: ${response.status}`);
      return response.json();
    })
    .then((payload) => (payload.documents ?? []).map(firestoreDocument))
    .catch((error) => {
      hqRunsSessionCache.delete(collectionName);
      throw error;
    });
  hqRunsSessionCache.set(collectionName, request);
  return request;
}

export function normalizeHqMoon(value) {
  const text = stripLcQuote(value).trim().replace(/\s+/, "-");
  return text || "41-Experimentation";
}

export function getPlayerCountFromStats(stats) {
  const players = valueAt(stats, "Players");
  if (players && typeof players === "object") return Math.max(1, Object.keys(players).length);
  return 1;
}

export function getLeaderboardInput(statsView, stats, settings = {}) {
  const isQuota = statsView?.newQuota !== 0;
  const boardType = isQuota ? "hq" : "sdc";
  const track = settings.track === "modded" ? "modded" : "vanilla";
  const metricKey = boardType === "hq" ? "quotaAmount" : "totalScrap";
  const score = boardType === "hq"
    ? intish(statsView?.newQuota)
    : intish(statsView?.totalAvailableValue || statsView?.collectedTotal);
  return {
    boardType,
    track,
    collectionName: HQ_LEADERBOARD_COLLECTIONS[track]?.[boardType] ?? HQ_COLLECTIONS[boardType],
    metricKey,
    score,
    playerCount: getPlayerCountFromStats(stats),
    version: `v${intish(valueAtAny(stats, ["Version"], 0))}`,
    moon: normalizeHqMoon(statsView?.moon ?? valueAt(stats, "MoonInfo.Name", "")),
  };
}

export function getRunMetric(run, input) {
  if (input.boardType === "sdc") {
    return intish(run.topLine ?? run.topline ?? run.totalAvailableValue ?? run.totalScrap);
  }
  return intish(run[input.metricKey]);
}

export function calculateLeaderboardCheck(runs, input, settings = {}) {
  const includeCurrentVersion = settings.includeCurrentVersion === true;
  const filtered = runs
    .filter((run) => run.verified === true)
    .filter((run) => input.boardType === "sdc" || String(run.players?.length || 0) === String(input.playerCount))
    .filter((run) => input.boardType === "hq" || !input.moon || run.moon === input.moon)
    .filter((run) => !includeCurrentVersion || normalizeVersion(run.version) === normalizeVersion(input.version))
    .sort((a, b) => getRunMetric(b, input) - getRunMetric(a, input));

  let rank = 1;
  for (const run of filtered) {
    if (getRunMetric(run, input) > input.score) rank += 1;
  }

  return {
    status: "ready",
    ...input,
    collections: HQ_LEADERBOARD_COLLECTIONS,
    boardTypes: HQ_LEADERBOARD_BOARD_TYPES,
    runFields: HQ_LEADERBOARD_RUN_FIELDS,
    totalRecords: filtered.length,
    top: filtered[0] ? {
      rank: 1,
      score: getRunMetric(filtered[0], input),
      players: filtered[0].players ?? [],
    } : null,
    includeCurrentVersion,
    metricLabel: input.boardType === "sdc" ? "Top Line" : "Score",
    next: filtered.find((run) => getRunMetric(run, input) < input.score) ?? null,
    nextScore: (() => {
      const next = filtered.find((run) => getRunMetric(run, input) < input.score);
      return next ? getRunMetric(next, input) : null;
    })(),
    rank,
  };
}

export function createLcStatsView(stats) {
  if (!stats || typeof stats !== "object") return stats ?? null;
  const aliases = {
    moon: () => stripLcQuote(valueAt(stats, "MoonInfo.Name", "Unknown")),
    seed: () => stripLcQuote(valueAt(stats, "Seed", "")),
    collectedTotal: () => intish(valueAtAny(stats, ["PerformanceInfo.CollectedTotal", "CollectedTotal"])),
    collectedNoExtra: () => intish(valueAtAny(stats, ["PerformanceInfo.CollectedNoExtra", "CollectedNoExtra"])),
    initialAvailableValue: () => intish(valueAtAny(stats, [
      "PerformanceInfo.InitialAvailableValue",
      "InitialAvailableValue",
      "BottomLine",
    ])),
    totalAvailableValue: () => intish(valueAtAny(stats, [
      "PerformanceInfo.TotalAvailableValue",
      "TotalAvailableValue",
      "BottomLineTrue",
    ])),
    valueSold: () => intish(valueAtAny(stats, ["QuotaInfo.ValueSold", "ValueSold"])),
    newQuota: () => intish(valueAtAny(stats, ["QuotaInfo.NewQuota", "NewQuota"])),
    lostScrap: () => {
      const missed = Array.isArray(stats.MissedItems) ? stats.MissedItems : [];
      return missed
        .filter((item) => item && item.CollectedOnPreviousDay)
        .reduce((total, item) => total + intish(item.Value), 0);
    },
  };
  aliases.isQuotaEvent = () => aliases.newQuota() !== 0;
  aliases.isSellOrQuotaEvent = () => aliases.valueSold() !== 0 || aliases.newQuota() !== 0;

  return new Proxy(stats, {
    get(target, prop, receiver) {
      if (typeof prop === "string" && aliases[prop]) return aliases[prop]();
      return Reflect.get(target, prop, receiver);
    },
  });
}
