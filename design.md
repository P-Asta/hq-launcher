# Design Notes

## Color Tokens

Use a small surface palette so the launcher keeps its original dark HQ feel.

- Page background: `var(--theme-bg)`
- Main panels, mod wrapper, modals, dropdowns, context menus: `var(--theme-surface)`
- Controls inside panels, list rows, inputs, secondary buttons: `bg-black/20`
- Selected rows and open/hover states: `bg-white/[0.07]` or `bg-white/[0.08]`
- Borders: `border-panel-outline`
- Primary/accent-only elements: `var(--theme-accent)`

Avoid adding new hard-coded dark backgrounds such as `#0f1116`, `#12141a`, or `#14161c`.

## Theme

Theme settings are based on hue and brightness:

- Hue changes `--theme-accent` and subtly tints the dark surfaces.
- Brightness only adjusts dark surface lightness.
- Default brightness is `0`.

The theme should feel like the original dark design with a little color added, not a fully recolored app.

## Controls

- Inputs, selects, secondary buttons, small utility buttons, and row-like controls should share `bg-black/20`.
- Major dialogs and menus should share `var(--theme-surface)`.
- The Start Run button is an exception and stays white for strong action contrast.
- Error states keep red. Success/progress states can use `var(--theme-accent)`.
