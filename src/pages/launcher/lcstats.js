import { GOOGLE_OAUTH_MOD_KEYS } from "./constants";
import { normalizeSheetColumn, normalizeSheetColumnList } from "./googleSheets";
import { modKeyLower } from "./mods";

export const LCSTATS_LAYOUTS = [
  "Custom Layout",
  // "AutoSheetModel",
  "BreadSheet",
  "WafrodyAutoSheet",
  // "SerenadeSheet",
  "CharlyAutoSheet",
  "EvieAutoSheet",
  "Evilsheet",
  "MakuSheet 1.0",
  "ModdedSheet",
];

export const LCSTATS_LAYOUT_COLUMN_FIELDS = new Set([
  "AutoSheetModel",
]);

export const DEFAULT_CUSTOM_LCSTATS_LAYOUT = {
  startRow: 3,
  checkColumn: "O",
  textCase: "Original",
  timeFormat: "12-hour",
  quotaColumn: "B",
  seedColumn: "",
  moonColumn: "F",
  weatherColumn: "G",
  layoutColumn: "H",
  itemCountColumn: "I",
  apparatusColumn: "",
  beeAmountColumn: "J",
  splitHiveCount: false,
  beehiveCollectedColumn: "",
  beehiveCollectedValueColumn: "",
  beehiveCollectedNotesEnabled: true,
  beeValueColumn: "K",
  cheapHiveColumn: "",
  expensiveHiveColumn: "",
  writeZeroForMissingHives: false,
  eggColumn: "L",
  eggNotesEnabled: false,
  availableEggValueColumn: "",
  availableOutdoorValueColumn: "",
  collectedEggColumn: "",
  collectedEggNotesEnabled: true,
  nutColumn: "M",
  nutCollectColumn: "",
  nutNotesEnabled: false,
  butlerColumn: "N",
  butlerCollectColumn: "",
  butlerNotesEnabled: false,
  collectedColumn: "O",
  availableColumn: "P",
  realAvailableColumn: "",
  collectedNoExtraColumn: "",
  missingColumn: "Q",
  filterCollectedGiftScrapFromMissing: true,
  outsideItemsColumn: "",
  soldColumn: "X",
  sidColumn: "Y",
  sidItemColumn: "",
  sidNotesEnabled: true,
  sidWriteFalse: false,
  infestationColumn: "Z",
  infestationWriteFalse: false,
  lostScrapColumn: "AB",
  takeoffTimeColumn: "",
  turretColumn: "",
  landmineColumn: "",
  spiketrapColumn: "",
  appLessColumn: "",
  deathColumns: "AC,AD,AE,AF",
  playerNameColumns: "",
  playerNameRow: 1,
  aliveState: "S",
  deadState: "X",
  missingState: "M",
  disconnectedState: "DC",
  lateDeadState: "SX",
  deathNotesEnabled: true,
  playerNamesAsNotes: false,
  deathEnemyNotesEnabled: false,
  enemyWriteFalse: false,
  enemyWriteZero: false,
  jesterColumn: "",
  barberColumn: "",
  bunkerSpiderColumn: "",
  brackenColumn: "",
  cadaverColumn: "",
  ghostGirlColumn: "",
  maneaterColumn: "",
  backwaterGunkfishColumn: "",
  coilHeadColumn: "",
  hoardingBugColumn: "",
  maskedColumn: "",
  snareFleaColumn: "",
  sporeLizardColumn: "",
  thumperColumn: "",
  earthLeviathanColumn: "",
  forestGiantColumn: "",
  baboonHawkColumn: "",
  oldBirdColumn: "",
  bushWolfColumn: "",
  feioparColumn: "",
  eyelessDogColumn: "",
  fogColumn: "AG",
  fogWriteFalse: false,
  meteorColumn: "AH",
  meteorWriteFalse: false,
  giftsColumn: "AI",
  giftBoxesNetOnly: false,
};

export const DEFAULT_LCSTATS_SETTINGS = {
  useLcstatsApi: true,
  spreadsheetId: "",
  activeSheetName: "",
  activeSheetId: "",
  startColumn: "D",
  quotaColumn: "B",
  sellColumn: "AE",
  layout: "AutoSheetModel",
  customLayout: DEFAULT_CUSTOM_LCSTATS_LAYOUT,
  googleClientId: "",
  googleClientSecret: "",
  googlePickerApiKey: "",
  googlePickerAppId: "",
  allowWithoutGoogle: false,
};

export const CUSTOM_TIME_FORMAT_OPTIONS = [
  { value: "12-hour", label: "7:40 AM" },
  { value: "12-hour compact", label: "7:40AM" },
  { value: "24-hour", label: "19:40" },
];

export function requiresGoogleOauthForMod(mod) {
  return GOOGLE_OAUTH_MOD_KEYS.has(modKeyLower(mod));
}

export function isLcStatsTrackerMod(mod) {
  return requiresGoogleOauthForMod(mod);
}

export function lcstatsLayoutUsesColumnFields(layout) {
  return LCSTATS_LAYOUT_COLUMN_FIELDS.has(layout);
}

export function hasCustomGoogleOauthSettings(settings) {
  return Boolean(
    String(settings?.googleClientId ?? "").trim() ||
      String(settings?.googleClientSecret ?? "").trim()
  );
}

export function normalizeCustomTimeFormat(value) {
  const text = String(value ?? "").trim();
  const normalized = text.toLowerCase().replace(/\s+/g, "");
  if (text === "7:40 AM") return "12-hour";
  if (normalized === "7:40am") return "12-hour compact";
  if (text === "19:40") return "24-hour";
  if (normalized === "keeporiginal" || normalized === "original") return "12-hour";
  return CUSTOM_TIME_FORMAT_OPTIONS.some((option) => option.value === text)
    ? text
    : "12-hour";
}

export function normalizeCustomLcstatsLayout(layout = {}) {
  const source = { ...DEFAULT_CUSTOM_LCSTATS_LAYOUT, ...(layout ?? {}) };
  const allowedTextCases = new Set([
    "Original",
    "UPPERCASE",
    "lowercase",
    "Title Case",
    "camelCase",
    "PascalCase",
  ]);
  const textCase = allowedTextCases.has(source.textCase)
    ? source.textCase
    : "Original";
  const timeFormat = normalizeCustomTimeFormat(source.timeFormat);
  return {
    ...source,
    startRow: Math.max(1, Math.floor(Number(source.startRow) || 1)),
    checkColumn: normalizeSheetColumn(source.checkColumn, ""),
    textCase,
    timeFormat,
    quotaColumn: normalizeSheetColumn(source.quotaColumn, ""),
    seedColumn: normalizeSheetColumn(source.seedColumn, ""),
    moonColumn: normalizeSheetColumn(source.moonColumn, ""),
    weatherColumn: normalizeSheetColumn(source.weatherColumn, ""),
    layoutColumn: normalizeSheetColumn(source.layoutColumn, ""),
    itemCountColumn: normalizeSheetColumn(source.itemCountColumn, ""),
    apparatusColumn: normalizeSheetColumn(source.apparatusColumn, ""),
    beeAmountColumn: normalizeSheetColumn(source.beeAmountColumn, ""),
    splitHiveCount: source.splitHiveCount === true,
    beehiveCollectedColumn: normalizeSheetColumn(source.beehiveCollectedColumn, ""),
    beehiveCollectedValueColumn: normalizeSheetColumn(
      source.beehiveCollectedValueColumn,
      "",
    ),
    beehiveCollectedNotesEnabled: source.beehiveCollectedNotesEnabled !== false,
    beeValueColumn: normalizeSheetColumn(source.beeValueColumn, ""),
    cheapHiveColumn: normalizeSheetColumn(source.cheapHiveColumn, ""),
    expensiveHiveColumn: normalizeSheetColumn(source.expensiveHiveColumn, ""),
    writeZeroForMissingHives: source.writeZeroForMissingHives === true,
    eggColumn: normalizeSheetColumn(source.eggColumn, ""),
    eggNotesEnabled: source.eggNotesEnabled === true,
    availableEggValueColumn: normalizeSheetColumn(source.availableEggValueColumn, ""),
    availableOutdoorValueColumn: normalizeSheetColumn(
      source.availableOutdoorValueColumn,
      "",
    ),
    collectedEggColumn: normalizeSheetColumn(source.collectedEggColumn, ""),
    collectedEggNotesEnabled: source.collectedEggNotesEnabled !== false,
    nutColumn: normalizeSheetColumn(source.nutColumn, ""),
    nutCollectColumn: normalizeSheetColumn(source.nutCollectColumn, ""),
    nutNotesEnabled:
      source.nutNotesEnabled === true || source.shotgunNotesEnabled === true,
    butlerColumn: normalizeSheetColumn(source.butlerColumn, ""),
    butlerCollectColumn: normalizeSheetColumn(source.butlerCollectColumn, ""),
    butlerNotesEnabled:
      source.butlerNotesEnabled === true || source.knifeNotesEnabled === true,
    collectedColumn: normalizeSheetColumn(source.collectedColumn, ""),
    availableColumn: normalizeSheetColumn(source.availableColumn, ""),
    realAvailableColumn: normalizeSheetColumn(source.realAvailableColumn, ""),
    collectedNoExtraColumn: normalizeSheetColumn(source.collectedNoExtraColumn, ""),
    missingColumn: normalizeSheetColumn(source.missingColumn, ""),
    filterCollectedGiftScrapFromMissing:
      source.filterCollectedGiftScrapFromMissing !== false,
    outsideItemsColumn: normalizeSheetColumn(source.outsideItemsColumn, ""),
    soldColumn: normalizeSheetColumn(source.soldColumn, ""),
    sidColumn: normalizeSheetColumn(source.sidColumn, ""),
    sidItemColumn: normalizeSheetColumn(source.sidItemColumn, ""),
    sidNotesEnabled: source.sidNotesEnabled !== false,
    sidWriteFalse: source.sidWriteFalse === true,
    infestationColumn: normalizeSheetColumn(source.infestationColumn, ""),
    infestationWriteFalse: source.infestationWriteFalse === true,
    lostScrapColumn: normalizeSheetColumn(source.lostScrapColumn, ""),
    takeoffTimeColumn: normalizeSheetColumn(source.takeoffTimeColumn, ""),
    turretColumn: normalizeSheetColumn(source.turretColumn, ""),
    landmineColumn: normalizeSheetColumn(source.landmineColumn, ""),
    spiketrapColumn: normalizeSheetColumn(source.spiketrapColumn, ""),
    appLessColumn: normalizeSheetColumn(
      source.appLessColumn ?? source.appyLessColumn,
      ""
    ),
    deathColumns: normalizeSheetColumnList(source.deathColumns, ""),
    playerNameColumns: normalizeSheetColumnList(source.playerNameColumns, ""),
    playerNameRow: Math.max(1, Math.floor(Number(source.playerNameRow) || 1)),
    aliveState: String(source.aliveState ?? "S"),
    deadState: String(source.deadState ?? "X"),
    missingState: String(source.missingState ?? "M"),
    disconnectedState: String(source.disconnectedState ?? "DC"),
    lateDeadState: String(source.lateDeadState ?? "SX"),
    deathNotesEnabled: source.deathNotesEnabled !== false,
    playerNamesAsNotes: source.playerNamesAsNotes === true,
    deathEnemyNotesEnabled: source.deathEnemyNotesEnabled === true,
    enemyWriteFalse: source.enemyWriteFalse === true,
    enemyWriteZero: source.enemyWriteZero === true,
    jesterColumn: normalizeSheetColumn(source.jesterColumn, ""),
    barberColumn: normalizeSheetColumn(source.barberColumn, ""),
    bunkerSpiderColumn: normalizeSheetColumn(source.bunkerSpiderColumn, ""),
    brackenColumn: normalizeSheetColumn(source.brackenColumn, ""),
    cadaverColumn: normalizeSheetColumn(source.cadaverColumn, ""),
    ghostGirlColumn: normalizeSheetColumn(source.ghostGirlColumn, ""),
    maneaterColumn: normalizeSheetColumn(source.maneaterColumn, ""),
    backwaterGunkfishColumn: normalizeSheetColumn(source.backwaterGunkfishColumn, ""),
    coilHeadColumn: normalizeSheetColumn(source.coilHeadColumn, ""),
    hoardingBugColumn: normalizeSheetColumn(source.hoardingBugColumn, ""),
    maskedColumn: normalizeSheetColumn(source.maskedColumn, ""),
    snareFleaColumn: normalizeSheetColumn(source.snareFleaColumn, ""),
    sporeLizardColumn: normalizeSheetColumn(source.sporeLizardColumn, ""),
    thumperColumn: normalizeSheetColumn(source.thumperColumn, ""),
    earthLeviathanColumn: normalizeSheetColumn(source.earthLeviathanColumn, ""),
    forestGiantColumn: normalizeSheetColumn(source.forestGiantColumn, ""),
    baboonHawkColumn: normalizeSheetColumn(source.baboonHawkColumn, ""),
    oldBirdColumn: normalizeSheetColumn(source.oldBirdColumn, ""),
    bushWolfColumn: normalizeSheetColumn(source.bushWolfColumn, ""),
    feioparColumn: normalizeSheetColumn(source.feioparColumn, ""),
    eyelessDogColumn: normalizeSheetColumn(source.eyelessDogColumn, ""),
    fogColumn: normalizeSheetColumn(source.fogColumn, ""),
    fogWriteFalse: source.fogWriteFalse === true,
    meteorColumn: normalizeSheetColumn(source.meteorColumn, ""),
    meteorWriteFalse: source.meteorWriteFalse === true,
    giftsColumn: normalizeSheetColumn(source.giftsColumn, ""),
    giftBoxesNetOnly: source.giftBoxesNetOnly === true,
  };
}

export function parseCustomLcstatsLayoutPreset(text) {
  const parsed = JSON.parse(text);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return null;
  }
  const allowedKeys = new Set(Object.keys(DEFAULT_CUSTOM_LCSTATS_LAYOUT));
  if (!Object.keys(parsed).some((key) => allowedKeys.has(key))) {
    return null;
  }
  return normalizeCustomLcstatsLayout(parsed);
}
