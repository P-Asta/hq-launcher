export const MOD_PANEL_WIDTH_STORAGE_KEY = "launcherModPanelWidthPercent";

export const LAUNCH_OPTIONS_STORAGE_KEY = "launcherLaunchOptions";

export const DEFAULT_MOD_PANEL_WIDTH = 40;

export const MIN_MOD_PANEL_WIDTH = 260;

export const MIN_CONFIG_PANEL_WIDTH = 320;

export const PRACTICE_LOCKED_MOD_KEYS = new Set([
  "hqhqteam::vlog",
  "asta::evlog",
]);

export const BASE_VLOG_MOD_KEY = "hqhqteam::vlog";

export const EVENT_VLOG_MOD = {
  dev: "asta",
  name: "EVlog",
  tags: [],
  enabled: true,
};

export const SMHQ_FORCED_MOD_KEYS = new Set([
  "slushyrh::freeeeeemoooooons",
]);

export const ECLIPSED_HQ_OPTIONAL_MOD_KEYS = new Set([
  "slushyrh::freeeeeemoooooons",
]);

export const ECLIPSED_FORCED_MOD_KEYS = new Set([
  "stormytuna::eclipseonly",
  "stormytuna::eclipsedonly",
]);

export const GOOGLE_OAUTH_MOD_KEYS = new Set([
  "mikuoreo::lcstatstracker",
]);

export const RUN_MODE_VALUES = [
  "hq",
  "smhq",
  "c_moons",
  "c_moons_smhq",
  "c_moons_eclipsed",
  "c_moons_practice",
  "practice",
  "eclipsed_hq",
  "brutal",
  "brutal_smhq",
  "brutal_eclipsed",
  "brutal_practice",
  "wesley",
  "wesley_smhq",
  "wesley_eclipsed",
  "wesley_practice",
];

export const DISCORD_DOWNLOAD_URL = "https://asta.rs/hq-launcher/";


export const SELECTED_EVENT_STORAGE_KEY = "selectedEventId";

/** Selectable run modes shown in the launch dropdown. */
export const RUN_OPTIONS = [
      {
        value: "hq",
        label: "HQ Run",
        buttonLabel: "HQ Run",
        preset: "hq",
        practice: false,
        title: "Normal run (HQ): practice mods are disabled",
      },
      {
        value: "smhq",
        label: "SMHQ Run",
        preset: "smhq",
        practice: false,
        title: "SMHQ preset run",
      },
      {
        value: "eclipsed_hq",
        label: "Eclipsed HQ",
        preset: "eclipsed_hq",
        practice: false,
        title: "HQ run with EclipsedOnly enabled",
      },
      {
        value: "practice",
        label: "Normal Practice",
        buttonLabel: "Normal Practice",
        preset: "hq",
        practice: true,
        title: "Practice run: installs/enables practice mods for this run",
      },
      {
        type: "separator",
        key: "run-group-brutal",
      },
      {
        value: "brutal",
        label: "Brutal Run",
        preset: "brutal",
        practice: false,
        title: "Brutal preset: installs Brutal-tagged mods (v49+)",
      },
      {
        value: "brutal_smhq",
        label: "Brutal SMHQ",
        preset: "brutal_smhq",
        practice: false,
        title:
          "Brutal + SMHQ preset: installs Brutal-tagged and SMHQ-tagged mods (v49+)",
      },
      {
        value: "brutal_eclipsed",
        label: "Brutal Eclipsed",
        preset: "brutal_eclipsed",
        practice: false,
        title: "Brutal preset with EclipsedOnly enabled",
      },
      {
        value: "brutal_practice",
        label: "Brutal Practice",
        preset: "brutal",
        practice: true,
        title:
          "Brutal preset: installs Brutal-tagged mods + practice mods (v49+)",
      },
      {
        type: "separator",
        key: "run-group-wesley",
      },
      {
        value: "wesley",
        label: "Wesley's Run",
        preset: "wesley",
        practice: false,
        title: "Wesley preset: installs Wesley-tagged mods (v69+)",
      },
      {
        value: "wesley_smhq",
        label: "Wesley's SMHQ",
        preset: "wesley_smhq",
        practice: false,
        title:
          "Wesley + SMHQ preset: installs Wesley-tagged and SMHQ-tagged mods (v69+)",
      },
      {
        value: "wesley_eclipsed",
        label: "Wesley Eclipsed",
        preset: "wesley_eclipsed",
        practice: false,
        title: "Wesley preset with EclipsedOnly enabled",
      },
      {
        value: "wesley_practice",
        label: "Wesley's Practice",
        preset: "wesley",
        practice: true,
        title:
          "Wesley preset: installs Wesley-tagged mods + practice mods (v69+)",
      },
      {
        type: "separator",
        key: "run-group-cmoons",
      },
      {
        value: "c_moons",
        label: "C.Moons Run",
        preset: "c_moons",
        practice: false,
        title: "C.Moons preset: installs C.Moons-tagged mods",
      },
      {
        value: "c_moons_smhq",
        label: "C.Moons SMHQ",
        preset: "c_moons_smhq",
        practice: false,
        title: "C.Moons + SMHQ preset: installs C.Moons-tagged and SMHQ-tagged mods",
      },
      {
        value: "c_moons_eclipsed",
        label: "C.Moons Eclipsed",
        preset: "c_moons_eclipsed",
        practice: false,
        title: "C.Moons preset with EclipsedOnly enabled",
      },
      {
        value: "c_moons_practice",
        label: "C.Moons Practice",
        preset: "c_moons",
        practice: true,
        title: "C.Moons preset: installs C.Moons-tagged mods + practice mods",
      },
      {
        type: "separator",
        key: "run-group-vanilla",
      },
      {
        value: "vanilla",
        label: "Vanilla Run",
        buttonLabel: "Vanilla Run",
        preset: "hq",
        practice: false,
        vanilla: true,
        title: "Vanilla run: launches without BepInEx or mods",
      },
    ];
