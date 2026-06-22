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
  valueAt(root: any, path: string | string[], fallback?: any): any;
  valueAtAny(root: any, paths: Array<string | string[]>, fallback?: any): any;
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
  consumePress(shortcut: OverlayShortcutString): boolean;
  consumeRelease(shortcut: OverlayShortcutString): boolean;
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
  valueAt(root: any, path: string | string[], fallback?: any): any;
  valueAtAny(root: any, paths: Array<string | string[]>, fallback?: any): any;
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
declare const valueAt: OverlayModuleApi["valueAt"];
declare const valueAtAny: OverlayModuleApi["valueAtAny"];
declare const intish: OverlayModuleApi["intish"];
