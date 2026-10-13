# Navigation

All five Aurora apps use the same controls. The header names the focused pane and shows numbered screens. Compact layouts give the focused pane the available space; hidden tabs have clickable overflow controls.

| Control | Behavior |
| --- | --- |
| Tab / Shift+Tab | Next / previous main screen outside text input |
| F6 / Shift+F6 | Next / previous pane |
| F2 | Search and select screens, current panes, and available selectors |
| Numbers | Select the screen shown in the header outside text input |
| Ctrl+K | Search actions; consequential actions retain their confirmations |
| Arrows / j / k | Select within the focused list |
| Page Up / Page Down | Page a list or scroll focused details |
| Enter | Open, inspect, or edit the selection |
| Space | Toggle a checkbox or selection where supported |
| Esc | Close the top overlay or leave editing; preserve drafts |
| F1 / ? | Help (question mark outside text input) |
| q / Ctrl+C | Request quit; running operations retain quit confirmations |
| Ctrl+X | Cancel a cancellable operation |

Dialogs, pickers, completion, and text editing receive keys before global navigation. Tab completes active Luma completion before switching screens. F6 lets you leave a text pane without losing its draft.

Click a screen or pane to focus it. Click list rows to select; double-click opens or edits where supported. The wheel moves the pane under the pointer. Confirmation buttons use the same guarded actions as the keyboard. Use `--no-mouse` for terminal text selection. Mouse capture is released on exit.

## Verification — 2026-10-13

- Normal suite: 37 tests passed.
- Every screen and pane rendered at 120×40, 100×30, 80×24, and 60×20; existing minimum-size, Unicode, empty/offline, input precedence, and confirmation checks pass.
- Keyboard cycling, remembered focus, F2 search/number selection, clickable navigation, action routing, and clipped hit targets have regression coverage. Consequential backend tests use mocks.
- Actual Foot captures cover the four sizes plus Navigation, Actions, and focused panes at wide and compact sizes. Pixel titles, header spacing, focus, shortcuts, and clipped content were visually inspected.
- Release build installed in `~/.local/bin/platter` and compared byte-for-byte with the build.

Captures: `/home/ken/Projects/work/tui-previews/mock-live/platter-*`. All screen/pane render artifacts: `/home/ken/Projects/work/tui-previews/navigation-matrix/platter/`.

Installed PTY verification: four sizes in default mouse mode and four with `--no-mouse` (40 checks across the five apps). Ctrl+K, F2, F6, screen navigation, resize, clean exit, termios, alternate-screen restoration, and mouse release passed. `--no-mouse` never enabled capture.

## Screenshot refresh and security themes — 2026-10-13

README screenshot recaptured in Foot at 120×40. Additional actual renders were inspected at 100×30, 80×24, and 60×20. Captures use sample data; consequential operations were not executed. Files: `/home/ken/Projects/work/tui-previews/crimson/platter-main-*.png`.

## Final shared pixel headers — 2026-10-13

All five titles use the same native terminal pixel geometry and size. Glim, Platter, and Luma preserve their accent-to-data gradients. Sweep and Breach use a solid theme accent so their title color matches focused borders and shortcut caps (Crimson #FF4D6D by default). Long titles preserve connection status above the pixels when it cannot fit beside them. Updated actual Foot screenshots: `/home/ken/Projects/work/tui-previews/final-pixel/`.

Final verification: `cargo fmt --check`, `cargo test --locked`, and `cargo build --release --locked` passed. Installed binaries match those release builds. Four-size PTY checks in mouse and `--no-mouse` modes passed for all five apps, including navigation, resizing, clean exit, termios, alternate-screen restoration, and mouse release (40 checks total).
