type OverlaySettingSchema =
  | { key: string; label: string; type: "boolean"; default?: boolean }
  | { key: string; label: string; type: "color"; default?: string }
  | { key: string; label: string; type: "number"; min?: number; max?: number; step?: number; default?: number }
  | { key: string; label: string; type: "range"; min: number; max: number; step?: number; default?: number }
  | { key: string; label: string; type: "text" | "textarea" | "key" | "image"; default?: string }
  | { key: string; label: string; type: "images"; default?: string[] }
  | { key: string; label: string; type: "select"; options: Array<{ label: string; value: string }>; default?: string };

/** Shortcut strings captured by key buttons, for example "Insert", "Ctrl+Shift+K", or "Ctrl+Shift+*". */
type OverlayShortcutString = string;

type OverlayRegisterType = "metadata" | "settings" | "defaults" | "css" | "visible" | "derive" | "renderOverlay" | "tick" | "lcstats";

type LeaderboardBoardType = "hq" | "sdc" | "smhq";
type LeaderboardTrack = "vanilla" | "modded";
type LeaderboardCollectionName =
  | "leaderboards_hq" | "leaderboards_sdc" | "leaderboards_smhq"
  | "modded_hq" | "modded_sdc" | "modded_smhq"
  | "lc_modded_brutal_hq" | "lc_modded_brutal_sdc" | "lc_modded_brutal_smhq"
  | "lc_modded_eclipsed_hq" | "lc_modded_eclipsed_smhq"
  | "lc_modded_wesleysmoons_hq" | "lc_modded_wesleysmoons_sdc" | "lc_modded_wesleysmoons_smhq"
  | "lc_modded_classicmoons_hq" | "lc_modded_classicmoons_sdc" | "lc_modded_classicmoons_smhq";

type LeaderboardRun = {
  id?: string;
  collectionName?: LeaderboardCollectionName | string;
  players?: string[];
  version?: string;
  verified?: boolean;
  quotaAmount?: number;
  quotaReached?: number;
  totalScrap?: number;
  moon?: string;
  scrapType?: string;
  videos?: any;
  date?: any;
  verifiedAt?: any;
  verifier?: string;
  [key: string]: any;
};

type LeaderboardState = {
  status: "idle" | "waiting" | "loading" | "ready" | "error";
  error?: string;
  reason?: string;
  track?: LeaderboardTrack;
  boardType?: LeaderboardBoardType;
  collectionName?: LeaderboardCollectionName | string;
  metricKey?: "quotaAmount" | "totalScrap" | string;
  metricLabel?: string;
  score?: number;
  rank?: number;
  totalRecords?: number;
  top?: { rank: number; score: number; players: string[] } | null;
  next?: LeaderboardRun | null;
  nextScore?: number | null;
  includeCurrentVersion?: boolean;
  playerCount?: number;
  version?: string;
  moon?: string;
  collections: {
    vanilla: Record<LeaderboardBoardType, LeaderboardCollectionName>;
    modded: Record<LeaderboardBoardType, LeaderboardCollectionName>;
    legacyModded: Record<string, Partial<Record<LeaderboardBoardType, LeaderboardCollectionName>>>;
  };
  boardTypes: Record<LeaderboardBoardType, { id: LeaderboardBoardType; name: string; metricLabel: string; metricKey: "quotaAmount" | "totalScrap" }>;
  runFields: string[];
};

type OverlayEndSummary = {
  id?: number;
  title?: string;
  lines?: string[];
  payload?: any;
  expiresAt?: number;
};

type OverlayStreamOverlays = {
  type?: string;
  messageType?: string;
  showOverlay?: boolean;
  crewCount?: number;
  moonName?: string;
  weatherName?: string;
  quotaValue?: number;
  quotaIndex?: number;
  lootValue?: number;
  [key: string]: any;
};

type OverlayScrapItem = {
  /** Normalized scrap display name, for example "Cash Register". */
  name: string;
  /** Scrap value. Missing or unparsable values become 0. */
  value: number;
  /** Original payload item for debugging or advanced modules. */
  raw?: any;
};

type OverlayScrapGroup = {
  /** Shared normalized item name for this group. */
  name: string;
  /** Individual values in this group, sorted high to low. */
  values: number[];
  /** Sum of all values in this group. */
  total: number;
  /** Highest individual value in this group. */
  max: number;
  /** Number of items in this group. */
  count: number;
  /** Original normalized scrap items in this group. */
  items: OverlayScrapItem[];
};

type OverlayScrapSummary = {
  /** Normalized moon/run name, or "Unknown" when unavailable. */
  moon: string;
  /** Normalized scrap left behind. Same value as remaining. */
  total: number | null;
  /** Normalized scrap left behind, or null when unavailable. */
  remaining: number | null;
  /** Missed/remaining scrap items, sorted high value first. */
  items: OverlayScrapItem[];
  /** Missed/remaining scrap items grouped by name, sorted by max desc, total desc, then name asc. */
  groups: OverlayScrapGroup[];
};

type OverlayScrapApi = {
  /** Full normalized end-run scrap summary. */
  summary(): OverlayScrapSummary;
  /** Shortcut for summary().moon. */
  moon(): string;
  /** Shortcut for summary().remaining. */
  remaining(): number | null;
  /** Alias for remaining(). */
  total(): number | null;
  /** Shortcut for summary().items. */
  items(): OverlayScrapItem[];
  /** Shortcut for summary().groups. */
  groups(): OverlayScrapGroup[];
};

type OverlayEnemyCounts = {
  /** Stable catalog id, for example "bracken". */
  id: string;
  /** Human display name, for example "Bracken". */
  name: string;
  /** LCStatsTracker code names and aliases matched for this enemy. */
  names: string[];
  /** Custom Layout style: bool enemies usually display presence, count enemies display counts. */
  kind: "bool" | "count" | string;
  /** Spawn source used for the count. */
  source: OverlayEnemySource;
  /** Best-effort killed count from available payloads. */
  killed: number;
  /** Spawn count from IndoorSpawns, DayTimeSpawns, and/or NightTimeSpawns. */
  spawned: number;
  /** Math.max(0, spawned - killed). */
  alive: number;
  /** True when spawned > 0. */
  present: boolean;
};

/** Spawn groups to read when counting enemies. */
type OverlayEnemySource = "all" | "indoor" | "inside" | "day" | "daytime" | "night" | "nighttime" | "outside";
/** Typed enemy ids, display names, and LCStatsTracker code names accepted by api.enemies helpers. */
type OverlayEnemyName =
  | "jester" | "Jester"
  | "barber" | "Barber" | "Clay Surgeon" | "ClaySurgeon"
  | "bunkerSpider" | "Bunker Spider" | "SandSpider"
  | "bracken" | "Bracken" | "Flowerman"
  | "cadaver" | "Cadaver" | "Cadaver Growth" | "Cadaver Growths"
  | "ghostGirl" | "Ghost Girl" | "Girl"
  | "maneater" | "Maneater" | "CaveDweller"
  | "backwaterGunkfish" | "Backwater Gunkfish" | "Stingray"
  | "coilHead" | "Coil Head" | "Spring"
  | "hoardingBug" | "Hoarding Bug" | "Hoarding bug"
  | "hygrodere" | "Hygrodere" | "Blob"
  | "masked" | "Masked" | "MaskedPlayerEnemy"
  | "snareFlea" | "Snare Flea" | "Centipede"
  | "sporeLizard" | "Spore Lizard" | "Puffer"
  | "thumper" | "Thumper" | "Crawler"
  | "nutcracker" | "Nutcracker"
  | "butler" | "Butler"
  | "manticoil" | "Manticoil" | "Mantacoil"
  | "roamingLocusts" | "Roaming Locusts" | "Docile Locust Bees"
  | "circuitBees" | "Circuit Bees" | "Red Locust Bees"
  | "tulipSnake" | "Tulip Snake" | "FlowerSnake"
  | "giantSapsucker" | "Giant Sapsucker" | "Giant Kiwi"
  | "earthLeviathan" | "Earth Leviathan"
  | "forestGiant" | "Forest Giant" | "ForestGiant"
  | "baboonHawk" | "Baboon Hawk" | "Baboon hawk"
  | "oldBird" | "Old Bird" | "RadMech"
  | "bushWolf" | "Bush Wolf" | "Kidnapper Fox"
  | "feiopar" | "Feiopar"
  | "eyelessDog" | "Eyeless Dog" | "MouthDog";

type OverlayEnemyQuery = {
  /** Optional stable id for your custom/modded enemy. */
  id?: string;
  /** Human display name for your custom/modded enemy. */
  name?: string;
  /** LCStatsTracker names/aliases to match exactly after normalization. */
  names: string[];
  /** Suggested display style for consumers of this query. */
  kind?: "bool" | "count" | string;
  /** Default source to use when no options.source override is supplied. */
  source?: OverlayEnemySource;
};

type OverlayEnemyDefinition = {
  /** Stable catalog id used by OverlayEnemyName. */
  id: string;
  /** Human display name. */
  name: string;
  /** LCStatsTracker code names and aliases. */
  names: string[];
  /** Custom Layout style. */
  kind: "bool" | "count" | string;
  /** Default source used by counts(). */
  source: "all" | "indoor" | "night" | string;
};

type OverlayEnemiesApi = {
  /** Known typed enemy catalog. Use this to render dynamic enemy lists. */
  list(): OverlayEnemyDefinition[];
  /** Full normalized counts for a known enemy name or custom query object. */
  counts(enemy: OverlayEnemyName | OverlayEnemyQuery, options?: { source?: OverlayEnemySource }): OverlayEnemyCounts;
  /** Shortcut for counts(...).spawned. */
  spawned(enemy: OverlayEnemyName | OverlayEnemyQuery, options?: { source?: OverlayEnemySource }): number;
  /** Shortcut for counts(...).killed. */
  killed(enemy: OverlayEnemyName | OverlayEnemyQuery, options?: { source?: OverlayEnemySource }): number;
  /** Shortcut for counts(...).alive. */
  alive(enemy: OverlayEnemyName | OverlayEnemyQuery, options?: { source?: OverlayEnemySource }): number;
  /** Shortcut for counts(...).present. */
  present(enemy: OverlayEnemyName | OverlayEnemyQuery, options?: { source?: OverlayEnemySource }): boolean;
  /** Convenience shortcut for counts("Butler"). */
  butler(options?: { source?: OverlayEnemySource }): OverlayEnemyCounts;
  /** Convenience shortcut for counts("Nutcracker"). */
  nutcracker(options?: { source?: OverlayEnemySource }): OverlayEnemyCounts;
};

type OverlayContext = {
  editMode: boolean;
  controlsOpen: boolean;
  elapsedSeconds: number;
  lcstats: any;
  lcstatsRaw: string | null;
  lcstatsPayload: { raw: string; stats: any } | null;
  lcstatsAgeMs: number | null;
  streamOverlays: OverlayStreamOverlays | null;
  streamOverlay: OverlayStreamOverlays | null;
  streamOverlaysAgeMs: number | null;
  displayTimeMs: number;
  leaderboard: LeaderboardState;
  /** @deprecated Use context.leaderboard. */
  recordChecker: LeaderboardState;
  endSummary: OverlayEndSummary | null;
  events: any[];
  inputSequence: number;
  formatSeconds(totalSeconds: number): string;
  escapeHtml(value: any): string;
  html(value: any): string;
  number(value: any): string;
  stripLcQuote(value: any): any;
  intish(value: any, fallback?: number): number;
  scrap: OverlayScrapApi;
  enemies: OverlayEnemiesApi;
};

type OverlayInputEvent = {
  id: string | number;
  type: "keydown" | "keyup";
  key: string;
  shortcut: OverlayShortcutString;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  source: "window" | "module-key" | "overlay-key" | "global-shortcut" | string;
  receivedAt: number;
};

type OverlayInputApi = {
  down(shortcut: OverlayShortcutString): boolean;
  held(shortcut: OverlayShortcutString): boolean;
  shortcut(shortcut: OverlayShortcutString): boolean;
  pressed(shortcut: OverlayShortcutString): boolean;
  released(shortcut: OverlayShortcutString): boolean;
  /** One-shot key down check. Optional scope lets one module bind the same key to multiple independent actions. */
  consumePress(shortcut: OverlayShortcutString, scope?: string): boolean;
  /** One-shot key release check. Optional scope lets one module bind the same key to multiple independent actions. */
  consumeRelease(shortcut: OverlayShortcutString, scope?: string): boolean;
  events(): OverlayInputEvent[];
  last(): OverlayInputEvent | null;
};

type OverlayHandlerArgs<TData = any> = {
  context: OverlayContext;
  data: TData;
  settings: Record<string, any>;
  config: any;
  api: OverlayModuleApi;
};

type OverlayModuleApi = {
  id: string;
  formatSeconds(totalSeconds: number): string;
  escapeHtml(value: any): string;
  html(value: any): string;
  number(value: any): string;
  stripLcQuote(value: any): any;
  intish(value: any, fallback?: number): number;
  scrap: OverlayScrapApi;
  enemies: OverlayEnemiesApi;
  className(name?: string): string;
  now(): number;
  input: OverlayInputApi;
  readonly context: OverlayContext | null;
  getLcStats(): any;
  getLcStatsRaw(): string | null;
  getStreamOverlay(): OverlayStreamOverlays | null;
};

type OverlayRenderItem = {
  id?: string;
  html: string | number | null | undefined;
  defaultPosition?: { x: number; y: number };
};

declare const Setting: {
  toggle(key: string, label: string, defaultValue?: boolean): OverlaySettingSchema;
  color(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  number(key: string, label: string, defaultValue?: number, min?: number, max?: number, step?: number): OverlaySettingSchema;
  range(key: string, label: string, min: number, max: number, step?: number, defaultValue?: number): OverlaySettingSchema;
  text(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  textarea(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  image(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  images(key: string, label: string, defaultValue?: string[]): OverlaySettingSchema;
  key(key: string, label: string, defaultValue?: OverlayShortcutString): OverlaySettingSchema;
  hotkey(key: string, label: string, defaultValue?: OverlayShortcutString): OverlaySettingSchema;
  select(key: string, label: string, options: Array<{ label: string; value: string }>, defaultValue?: string): OverlaySettingSchema;
  selectMenu(key: string, label: string, options: Array<{ label: string; value: string }>, defaultValue?: string): OverlaySettingSchema;
};

declare function register(type: "settings", payload: OverlaySettingSchema[]): unknown;
declare function register(type: "defaults" | "metadata", payload: Record<string, any>): unknown;
declare function register(type: "css", payload: string): unknown;
declare function register(type: "visible", payload: (args: OverlayHandlerArgs) => boolean | void): unknown;
declare function register<TData = any>(type: "derive", payload: (args: OverlayHandlerArgs) => TData): unknown;
declare function register<TData = any>(type: "renderOverlay", payload: (args: OverlayHandlerArgs<TData>) => string | number | null | undefined | OverlayRenderItem[]): unknown;
declare function register<TData = any>(type: "tick" | "lcstats", payload: (args: OverlayHandlerArgs<TData>) => unknown): unknown;
declare function register(type: OverlayRegisterType, payload: any): unknown;

declare function setName(name: string): unknown;
declare function setDescription(description: string): unknown;
declare function setLocked(locked?: boolean): unknown;
declare function setDefaultPosition(position: { x: number; y: number }): unknown;
declare function setDefaultSettings(settings: Record<string, any>): unknown;
declare function setWrapperClass(wrapperClass: string): unknown;
declare function setCss(css: string): unknown;

declare const api: OverlayModuleApi;
declare const html: OverlayModuleApi["html"];
declare const formatSeconds: OverlayModuleApi["formatSeconds"];
declare const number: OverlayModuleApi["number"];
declare const intish: OverlayModuleApi["intish"];
