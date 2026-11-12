# Responsive launcher correction

User wants the existing HQ launcher's UI to respond to narrow widths, with Android-specific runtime capabilities, rather than a separate mobile design.

1. Extract the existing workspace, toolbar and mod row presentation for desktop and Android to share. Preserve desktop commands. Remove the Android hero/onboarding layout.
2. At widths below 800px wrap the toolbar and stack panels without fixed minimum widths or resize handles. Width controls layout; Android controls runtime actions only. Keep installation, progress/errors, mod switches and microphone access.
3. Verify browser actions and resizing at 320/412/795/915/1280px, desktop build and Android build. Update installed APK preserving all data and inspect on phone.
4. Finish the ongoing touch work: reproduce/fix GUI drag-origin and pointer-handoff findings with JVM tests, retain v80-only Tab. Rebuild once both changes are frozen.

Remaining runtime limitation: vanilla rendered Launch Mode; modded startup black remains unresolved. Vanilla diagnostic and CullFactory preference were restored before this correction.

Result: shared presentation and responsive correction verified in actual desktop/Android browser fixtures and final installed APK. Final phone384x779: compact=true,20 mod rows, shared search, one toolbar button row, no horizontal/panel overflow, no hero. MainActivity visible and own AndroidRuntime error output empty. Native input regressions pass JVM tests; full gameplay touch response and modded black startup remain open.
