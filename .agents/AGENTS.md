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
