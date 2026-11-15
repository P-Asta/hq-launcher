use std::collections::BTreeMap;

use crate::mod_config::ModEntry;

/// Package identities only: versions are always resolved from the latest Thunderstore listing.
pub fn get_diomedes_mod_list() -> Vec<ModEntry> {
    [
        ("BepInEx", "BepInExPack"),
        ("Lordfirespeed", "OdinSerializer"),
        ("Rune580", "LethalCompany_InputUtils"),
        ("Hamunii", "DetourContext_Dispose_Fix"),
        ("mattymatty", "MonkeyInjectionLibrary"),
        ("Hamunii", "AutoHookGenPatcher"),
        ("Evaisa", "FixPluginTypesSerialization"),
        ("Zaggy1024", "PathfindingLib"),
        ("DiFFoZ", "HarmonyXTranspilerFix"),
        ("DiFFoZ", "DiFFoZTweaks"),
        ("ButteryStancakes", "SafeSoloScrap"),
        ("FlipMods", "FasterItemDropship"),
        (
            "dotnet_lethal_company",
            "System_Runtime_CompilerServices_Unsafe",
        ),
        ("dotnet_lethal_company", "System_Memory"),
        ("dotnet_lethal_company", "System_Threading_Channels"),
        (
            "dotnet_lethal_company",
            "System_Diagnostics_DiagnosticSource",
        ),
        ("dotnet_lethal_company", "Serilog"),
        ("Sigurd", "Serilog_Pack"),
        ("DiFFoZ", "BepInEx_MonoMod_Debug_Patcher"),
        ("dotnet_lethal_company", "Serilog_Exceptions"),
        ("dotnet_lethal_company", "Serilog_Sinks_Map"),
        ("dotnet_lethal_company", "Serilog_Sinks_File"),
        ("dotnet_lethal_company", "System_Reflection_TypeExtensions"),
        ("quackandcheese", "ToggleMute"),
        ("Hardy", "LCMaxSoundsFix"),
        ("Zaggy1024", "PathfindingLagFix"),
        ("Chboo1", "High_Quota_Fixes"),
        ("TeamXiaolan", "DawnLib"),
        ("fumiko", "CullFactory"),
        ("Zaggy1024", "DunGenReferenceFixer"),
        ("DiFFoZ", "LethalPerformance"),
        ("mattymatty", "EntranceTeleportOptimizations"),
        ("LethalCompanyModding", "LCUltrawide_Community"),
        ("mattymatty", "LobbyControl"),
        ("DiomedesLC", "Diomedes"),
    ]
    .into_iter()
    .map(|(dev, name)| ModEntry {
        dev: dev.into(),
        name: name.into(),
        tags: vec!["Diomedes".into()],
        enabled: true,
        switch_group: None,
        low_cap: None,
        high_cap: None,
        tag_constraints: BTreeMap::new(),
        version_config: BTreeMap::new(),
    })
    .collect()
}

// Practice-run mods (Imperium, LethalDevMode, ...) live in manifest.json under `practice_mods`.

/// LCF - asta
/// Installed and enabled only for the LCF run mode; hardcoded here instead of the manifest.
pub fn get_lcf_mod_list() -> Vec<ModEntry> {
    vec![ModEntry {
        dev: "asta".to_string(),
        name: "LCF".to_string(),
        tags: vec![],
        enabled: true,
        switch_group: None,
        low_cap: None,
        high_cap: None,
        tag_constraints: BTreeMap::new(),
        version_config: BTreeMap::new(),
    }]
}
