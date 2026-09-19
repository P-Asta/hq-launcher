import { overlayDataSources } from "./scrap";
import { arrayValue, intish, pickMatchingKey } from "./values";

export function hasEnemyName(value, pattern) {
  if (typeof value === "string") return pattern.test(value);
  if (!value || typeof value !== "object") return false;
  return pattern.test(String(pickMatchingKey(value, /^(enemyName|name|type|enemy)$/i) ?? ""));
}

export function countEnemyKeys(root, pattern, modePattern) {
  if (!root || typeof root !== "object") return null;
  const seen = new Set();
  const stack = [root];
  let total = 0;
  let found = false;

  while (stack.length > 0) {
    const value = stack.pop();
    if (!value || typeof value !== "object" || seen.has(value)) continue;
    seen.add(value);

    for (const [key, child] of Object.entries(value)) {
      if (pattern.test(key) && modePattern.test(key) && typeof child !== "object") {
        total += intish(child);
        found = true;
        continue;
      }
      if (modePattern.test(key) && child && typeof child === "object" && !Array.isArray(child)) {
        for (const [enemyKey, enemyValue] of Object.entries(child)) {
          if (pattern.test(enemyKey) && typeof enemyValue !== "object") {
            total += intish(enemyValue);
            found = true;
          }
        }
      }
      if (child && typeof child === "object") stack.push(child);
    }
  }

  return found ? total : null;
}

export function countEnemyArrays(root, pattern, modePattern) {
  const seen = new Set();
  const stack = [{ value: root, inContainer: false }];
  let count = 0;
  let foundContainer = false;

  while (stack.length > 0) {
    const entry = stack.pop();
    const value = entry?.value;
    if (!value || typeof value !== "object" || seen.has(value)) continue;
    seen.add(value);

    if (Array.isArray(value)) {
      if (entry.inContainer) {
        foundContainer = true;
        for (const item of value) {
          if (hasEnemyName(item, pattern)) count += 1;
          if (item && typeof item === "object") stack.push({ value: item, inContainer: true });
        }
      } else {
        for (const item of value) {
          if (item && typeof item === "object") stack.push({ value: item, inContainer: false });
        }
      }
      continue;
    }

    for (const [key, child] of Object.entries(value)) {
      if (modePattern.test(key) && Array.isArray(child)) {
        foundContainer = true;
        for (const item of child) {
          if (hasEnemyName(item, pattern)) count += 1;
        }
      }
      stack.push({ value: child, inContainer: modePattern.test(key) });
    }
  }

  return foundContainer || count > 0 ? count : null;
}

export const OVERLAY_ENEMY_CATALOG = [
  { id: "jester", name: "Jester", names: ["Jester"], kind: "bool", source: "all" },
  { id: "barber", name: "Barber", names: ["Clay Surgeon", "ClaySurgeon"], kind: "bool", source: "all" },
  { id: "bunkerSpider", name: "Bunker Spider", names: ["Bunker Spider", "SandSpider"], kind: "bool", source: "all" },
  { id: "bracken", name: "Bracken", names: ["Flowerman"], kind: "bool", source: "all" },
  { id: "cadaver", name: "Cadaver", names: ["Cadaver Growths", "Cadaver Growth"], kind: "bool", source: "all" },
  { id: "ghostGirl", name: "Ghost Girl", names: ["Girl"], kind: "bool", source: "all" },
  { id: "maneater", name: "Maneater", names: ["Maneater", "CaveDweller"], kind: "bool", source: "all" },
  { id: "backwaterGunkfish", name: "Backwater Gunkfish", names: ["Stingray"], kind: "count", source: "all" },
  { id: "coilHead", name: "Coil Head", names: ["Spring"], kind: "count", source: "all" },
  { id: "hoardingBug", name: "Hoarding Bug", names: ["Hoarding bug", "Hoarding Bug"], kind: "count", source: "all" },
  { id: "hygrodere", name: "Hygrodere", names: ["Blob", "Hygrodere"], kind: "count", source: "indoor" },
  { id: "masked", name: "Masked", names: ["MaskedPlayerEnemy", "Masked"], kind: "count", source: "all" },
  { id: "snareFlea", name: "Snare Flea", names: ["Centipede"], kind: "count", source: "all" },
  { id: "sporeLizard", name: "Spore Lizard", names: ["Puffer"], kind: "count", source: "all" },
  { id: "thumper", name: "Thumper", names: ["Crawler"], kind: "count", source: "all" },
  { id: "nutcracker", name: "Nutcracker", names: ["Nutcracker"], kind: "count", source: "indoor" },
  { id: "butler", name: "Butler", names: ["Butler"], kind: "count", source: "indoor" },
  { id: "manticoil", name: "Manticoil", names: ["Manticoil", "Mantacoil"], kind: "count", source: "day" },
  { id: "roamingLocusts", name: "Roaming Locusts", names: ["Roaming Locusts", "Docile Locust Bees"], kind: "count", source: "day" },
  { id: "circuitBees", name: "Circuit Bees", names: ["Circuit Bees", "Red Locust Bees"], kind: "count", source: "day" },
  { id: "tulipSnake", name: "Tulip Snake", names: ["Tulip Snake", "FlowerSnake"], kind: "count", source: "day" },
  { id: "giantSapsucker", name: "Giant Sapsucker", names: ["Giant Sapsucker", "Giant Kiwi"], kind: "count", source: "day" },
  { id: "earthLeviathan", name: "Earth Leviathan", names: ["Earth Leviathan"], kind: "count", source: "night" },
  { id: "forestGiant", name: "Forest Giant", names: ["ForestGiant"], kind: "count", source: "night" },
  { id: "baboonHawk", name: "Baboon Hawk", names: ["Baboon hawk"], kind: "count", source: "night" },
  { id: "oldBird", name: "Old Bird", names: ["RadMech", "Old Bird"], kind: "count", source: "night" },
  { id: "bushWolf", name: "Bush Wolf", names: ["Bush Wolf"], kind: "bool", source: "night" },
  { id: "feiopar", name: "Feiopar", names: ["Feiopar"], kind: "count", source: "night" },
  { id: "eyelessDog", name: "Eyeless Dog", names: ["MouthDog", "Eyeless Dog"], kind: "count", source: "night" },
];

export function normalizedEnemyKey(value) {
  return String(value ?? "").toLowerCase().replace(/[^a-z0-9]/g, "");
}

export function resolveEnemyDefinition(value) {
  if (value && typeof value === "object" && Array.isArray(value.names)) {
    return {
      id: value.id ?? normalizedEnemyKey(value.name ?? value.names[0]),
      name: value.name ?? value.names[0],
      names: value.names.map(String),
      kind: value.kind ?? "count",
      source: value.source ?? "all",
    };
  }
  const key = normalizedEnemyKey(value);
  const found = OVERLAY_ENEMY_CATALOG.find((enemy) =>
    normalizedEnemyKey(enemy.id) === key
    || normalizedEnemyKey(enemy.name) === key
    || enemy.names.some((name) => normalizedEnemyKey(name) === key)
  );
  if (found) return found;
  const name = String(value ?? "");
  return { id: normalizedEnemyKey(name), name, names: [name], kind: "count", source: "all" };
}

export function enemyNameMatches(value, names) {
  const key = normalizedEnemyKey(value);
  return names.some((name) => normalizedEnemyKey(name) === key);
}

export function spawnGroupNames(source) {
  switch (String(source ?? "all").toLowerCase()) {
    case "indoor":
    case "inside":
      return ["IndoorSpawns"];
    case "day":
    case "daytime":
      return ["DayTimeSpawns"];
    case "night":
    case "outside":
    case "nighttime":
      return ["NightTimeSpawns"];
    case "all":
    default:
      return ["IndoorSpawns", "DayTimeSpawns", "NightTimeSpawns"];
  }
}

export function countEnemySpawnArrays(source, names, sourceMode) {
  if (!source || typeof source !== "object") return null;
  let found = false;
  let total = 0;
  for (const group of spawnGroupNames(sourceMode)) {
    const values = arrayValue(source[group]);
    if (values.length > 0) found = true;
    total += values.filter((spawn) => enemyNameMatches(spawn?.Enemy ?? spawn?.enemy ?? spawn?.name, names)).length;
  }
  return found ? total : null;
}

export function enemyCountFromSources(sources, pattern, modePattern) {
  let total = 0;
  let found = false;
  for (const source of sources) {
    const keyed = countEnemyKeys(source, pattern, modePattern);
    if (keyed !== null) {
      total += keyed;
      found = true;
      continue;
    }
    const arrayCount = countEnemyArrays(source, pattern, modePattern);
    if (arrayCount !== null) {
      total += arrayCount;
      found = true;
    }
  }
  return found ? total : 0;
}

export function normalizedEnemyCounts(context, enemy, options = {}) {
  const sources = overlayDataSources(context);
  const definition = resolveEnemyDefinition(enemy);
  const sourceMode = options.source ?? definition.source ?? "all";
  const exactSpawned = sources
    .map((source) => countEnemySpawnArrays(source, definition.names, sourceMode))
    .find((count) => count !== null);
  const pattern = new RegExp(definition.names.map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|"), "i");
  const killed = enemyCountFromSources(sources, pattern, /kill|killed|dead|slain|defeat|defeated/i);
  let spawned = exactSpawned ?? enemyCountFromSources(sources, pattern, /spawn|spawned|enemy|enemies|creature|monster|outside|inside/i);
  if (spawned === 0 && killed > 0) spawned = killed;
  return {
    id: definition.id,
    name: definition.name,
    names: definition.names.slice(),
    kind: definition.kind,
    source: sourceMode,
    killed,
    spawned,
    alive: Math.max(0, spawned - killed),
    present: spawned > 0,
  };
}
