# Rust Island Agent Rules

## Slint Conventions
- All panels inherit from `Rectangle` and fill 100% width/height
- Panels never implement their own close buttons — global dismiss is in island.slint
- Use Theme.* tokens for all colors — never hardcode `#ff453a` etc.
- Use theme spacing constants — never hardcode padding values
- Every TouchArea must have minimum 32px width and height (accessibility)
- Never use emoji characters as icons — use SVG files from assets/icons/

## Color Rules
- Maximum 1 accent color per panel at rest
- A second color is acceptable for semantic meaning (red for delete, green for success)
- Default icon color is `text-secondary` (rgba white 55% opacity)
- Active icon color is `text-primary` (rgba white 95% opacity)

## Backend Conventions
- One backend module = one system resource (audio, wifi, bluetooth, etc.)
- All background loops check `active_section` to skip work when their panel isn't visible
- All UI updates from background threads use `slint::invoke_from_event_loop`
- Log with `crate::backend::log_to_file()`, never `println!()`

## File Organization
- Components: src/ui/components/ — max 50 lines each
- Panel shells: src/ui/panels/<feature>/panel.slint — max 150 lines
- Panel sub-elements: src/ui/panels/<feature>/<element>.slint — max 100 lines

## Theme Token Rules
- ALWAYS use V3 semantic tokens (Theme.color-red, Theme.color-blue, etc.)
- NEVER use legacy aliases (Theme.accent-primary, Theme.accent-error, etc.)
- NEVER hardcode hex colors (#ffffff, #000000, #ff453a) — use Theme.text-primary, Theme.surface-primary, Theme.color-red
- NEVER hardcode rgba(...) values for common colors — use the Theme token
- The ONLY acceptable hardcoded color is `transparent`
- For alpha-modified theme colors, use Slint's color functions if available, or define a new token

## Spacing Rules
- ALWAYS use Theme.panel-padding for panel outer padding
- ALWAYS use Theme.panel-padding-bottom for panels with dock space
- ALWAYS use Theme.spacing-* scale (xs=4, sm=8, md=12, lg=16, xl=20) for spacing
- NEVER hardcode pixel values for padding or spacing

## Component Reuse Rules
- Use SearchField component for ALL search inputs — never build inline
- Use SectionHeader component for ALL panel headers — never build inline
- Use EmptyState component for ALL empty list states — never show raw text
- Use SliderControl for ALL sliders — never use std-widgets Slider
- Use IconButton for ALL interactive icon buttons
- Use PillTabs for ALL segmented controls / tab bars
- Use GlassCard for ALL card containers

## Dead Code Rules
- When a file is replaced by a new version in a subdirectory, DELETE the old file immediately
- Never leave deprecated files "for reference" — use git history instead
- If island.slint doesn't import a panel file, that file is dead code

## Inline Documentation
- Every .slint file MUST have a 3-line comment block at the top:
  // PURPOSE: What this file renders
  // INPUTS: Key properties and callbacks  
  // USED BY: Parent file(s) that import this

## Animation Rules
- Equalizer/visualizer bars use ONE color only (Theme.text-tertiary) — motion creates interest, not color
- All state transitions must have animate blocks with Theme.micro-duration
- Never use raw duration values (150ms, 200ms) — use Theme timing tokens
