//! The window's own sheet: the layout of the shell, the pane and the sidebar's rows. quire has no
//! column component at v0.3.1, and a `RawKeySurface` takes no style of its own. Colours and type
//! come from quire's tokens and the pane's palette, so this sheet has no design values.

pub const WINDOW_CSS: &str = "\
.temor-shell { width: 100%; height: 100%; }\
.temor-window { display: flex; flex-direction: column; width: 100%; height: 100%; }\
.temor-content { flex: 1 1 auto; min-height: 0; min-width: 0; position: relative; }\
.temor-pane { display: flex; flex-direction: column; width: 100%; height: 100%; position: relative; }\
.temor-find { position: absolute; top: var(--s-2); right: var(--s-2); display: flex; align-items: center; gap: var(--s-2); padding: var(--s-2); background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-item); box-shadow: var(--shadow-sheet); }\
.temor-find-field { width: calc(var(--s-16) * 3); }\
.ds-palette { --sel-bg: var(--fill); --sel-ink: var(--ink); }\
.temor-status { display: flex; align-items: center; gap: var(--s-3); padding: 0 var(--s-3); min-width: 0; overflow: hidden; white-space: nowrap; flex: 0 0 auto; }\
.temor-keys { flex: 1 1 auto; min-height: 0; position: relative; overflow: hidden; }\
.temor-grid { position: relative; width: 100%; height: 100%; }\
.temor-block-header { position: absolute; top: 0; left: 0; right: 0; box-sizing: border-box; display: flex; align-items: center; min-width: 0; overflow: hidden; background: var(--raise); border-bottom: var(--hair) solid var(--line); }\
.temor-fact-chips { position: absolute; left: 0; right: 0; box-sizing: border-box; display: flex; align-items: center; justify-content: flex-end; gap: var(--s-2); pointer-events: none; overflow: hidden; }\
.temor-block-tools { position: absolute; display: flex; align-items: center; gap: var(--s-2); padding: var(--s-1) var(--s-2); background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-panel); }\
.temor-peek { position: absolute; box-sizing: content-box; padding: var(--s-16); pointer-events: none; overflow: hidden; background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-panel); box-shadow: var(--shadow-sheet); }\
.temor-preedit { position: absolute; left: 0; bottom: 0; white-space: pre; text-decoration: underline; }\
.temor-quick { position: absolute; left: 8%; right: 8%; top: 4%; bottom: 24%; display: flex; flex-direction: column; background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-panel); box-shadow: var(--shadow-sheet); overflow: hidden; }\
.temor-quick-head { display: flex; align-items: center; justify-content: flex-end; gap: var(--s-3); padding: var(--s-3); }\
.temor-quick-body { flex: 1 1 auto; min-height: 0; position: relative; }\
.temor-side-box { width: 100%; height: 100%; }\
.temor-side { width: 100%; height: 100%; }\
.temor-tab { display: flex; align-items: center; gap: var(--s-3); min-width: 0; }\
.temor-tab-words { display: flex; flex-direction: column; flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; }\
.temor-mini { position: relative; width: 100%; aspect-ratio: 16 / 9; min-height: calc(var(--s-5) * 5); }\
.temor-tile-slot { position: absolute; box-sizing: border-box; }\
.temor-tile-slot > * { display: block; width: 100%; height: 100%; }\
.temor-host { display: flex; flex-direction: column; flex: 0 0 auto; gap: var(--s-2); }\
.temor-host-line { display: flex; align-items: center; gap: var(--s-3); min-width: 0; }\
.temor-tile { box-sizing: border-box; width: 100%; height: 100%; border: var(--hair) solid var(--line); }\
.temor-tile[data-selected] { border-color: var(--accent); }\
.temor-dl-box { width: 100%; }\
.temor-dl { display: flex; flex-direction: column; gap: var(--s-3); min-width: 0; }\
.temor-dl-actions { display: flex; gap: var(--s-3); }";
