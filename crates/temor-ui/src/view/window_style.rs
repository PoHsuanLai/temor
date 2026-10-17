//! The window's own sheet: the layout of the shell, the pane and the sidebar's rows. quire has no
//! column component at v0.3.1, and a `RawKeySurface` takes no style of its own. Colours and type
//! come from quire's tokens and the pane's palette, so this sheet has no design values.

pub const WINDOW_CSS: &str = "\
.temor-shell { position: relative; width: 100%; height: 100%; }\
.temor-edge-host { position: absolute; left: 0; top: 0; bottom: 0; width: 0; }\
.temor-content { flex: 1 1 auto; min-height: 0; min-width: 0; position: relative; }\
.temor-pane { position: relative; display: flex; flex-direction: column; width: 100%; height: 100%; }\
.temor-edge { position: absolute; left: 0; right: 0; bottom: 0; display: flex; align-items: center; gap: var(--s-4); padding: 0 var(--s-4); pointer-events: none; }\
.temor-link-tip { position: absolute; width: var(--s-1); height: var(--s-16); pointer-events: none; }\
.temor-link-tip-spot { display: block; width: var(--s-1); height: var(--s-16); }\
.temor-find { position: absolute; top: var(--s-4); right: var(--s-4); display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4); background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-item); box-shadow: var(--shadow-sheet); }\
.temor-find-field { width: calc(var(--s-16) * 12); }\
.temor-first-hint { position: absolute; pointer-events: none; white-space: nowrap; }\
.temor-keys { flex: 1 1 auto; min-height: 0; position: relative; overflow: hidden; }\
.temor-grid { position: relative; width: 100%; height: 100%; overflow: hidden; }\
.temor-grid-slice { position: absolute; left: 0; right: 0; pointer-events: none; }\
.temor-row-gap { position: absolute; left: 0; right: 0; pointer-events: none; background: var(--term-bg); }\
.temor-block-header { position: absolute; top: 0; left: 0; right: 0; box-sizing: border-box; display: flex; align-items: center; min-width: 0; overflow: hidden; background: var(--raise); border-bottom: var(--hair) solid var(--line); }\
.temor-lens-layer { position: absolute; top: 0; left: 0; right: 0; bottom: 0; overflow: hidden; pointer-events: none; }\
.temor-lens { position: absolute; display: block; object-fit: contain; object-position: left top; }\
.temor-fact-chips { position: absolute; left: 0; right: 0; box-sizing: border-box; display: flex; align-items: center; justify-content: flex-end; gap: var(--s-6); pointer-events: none; overflow: hidden; }\
.temor-fact-chips[data-place=over], .temor-fact-chips[data-place=header] { justify-content: flex-start; background: var(--term-bg); }\
.temor-chip { display: flex; flex: none; align-items: center; gap: var(--s-5); box-sizing: border-box; height: calc(100% - var(--s-2)); max-height: var(--s-22); padding: 0 var(--s-6); border-radius: var(--r-micro); background: var(--fill-quaternary); border: var(--hair) solid var(--line-soft); color: var(--ink-soft); font-family: var(--font-code); font-size: var(--fs-small); white-space: nowrap; }\
.temor-chip[data-kind=folder] { flex: 0 1 auto; min-width: 0; }\
.temor-chip-words { display: flex; gap: 0.6ch; min-width: 0; overflow: hidden; }.temor-chip-words > span { flex: none; }.temor-chip[data-kind=folder] .temor-chip-words > span { flex: 0 1 auto; min-width: 0; overflow: hidden; }\
.temor-chip-icon { display: flex; color: var(--chip-hue); }.temor-chip [data-tone=strong] { color: var(--chip-hue); font-weight: 600; }\
.temor-chip { --chip-hue: var(--ink); }.temor-chip[data-kind=folder] { --chip-hue: var(--term-ansi-12); }.temor-chip[data-kind=branch] { --chip-hue: var(--term-ansi-13); }.temor-chip[data-kind=changes] { --chip-hue: var(--ink-soft); }.temor-chip[data-kind=new] { --chip-hue: var(--term-ansi-3); color: var(--chip-hue); }\
.temor-chip[data-kind=python] { --chip-hue: var(--term-ansi-11); }.temor-chip[data-kind=ruby] { --chip-hue: var(--term-ansi-9); }.temor-chip[data-kind=go] { --chip-hue: var(--term-ansi-14); }.temor-chip[data-kind=node] { --chip-hue: var(--term-ansi-10); }.temor-chip [data-tone=added] { color: var(--ok); }.temor-chip [data-tone=removed] { color: var(--danger); }\
.temor-block-tools { position: absolute; display: flex; align-items: center; gap: var(--s-4); }.temor-block-more { display: flex; }.temor-card-meta { position: absolute; display: flex; align-items: center; white-space: nowrap; pointer-events: none; }\
.temor-keys:focus { outline: none; }\
.temor-peek { position: absolute; box-sizing: content-box; padding: var(--s-16); pointer-events: none; overflow: hidden; background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-panel); box-shadow: var(--shadow-sheet); }\
.temor-preedit { position: absolute; left: 0; bottom: 0; white-space: pre; text-decoration: underline; }\
.temor-quick { position: absolute; left: 8%; right: 8%; top: 4%; bottom: 24%; display: flex; flex-direction: column; background: var(--raise); border: var(--hair) solid var(--line); border-radius: var(--r-panel); box-shadow: var(--shadow-sheet); overflow: hidden; }\
.temor-quick-head { display: flex; align-items: center; justify-content: flex-end; gap: var(--s-8); padding: var(--s-8); }\
.temor-quick-body { flex: 1 1 auto; min-height: 0; position: relative; }\
.temor-side-box { width: 100%; height: 100%; }\
.temor-side { width: 100%; height: 100%; }\
.temor-tab-box { position: relative; display: flex; width: 100%; min-width: 0; }\
.temor-tab-glyph { display: grid; place-items: center; }\
.temor-tab-glyph[*|data-tone=succeeded] { color: var(--ok); }\
.temor-tab-glyph[*|data-tone=failed] { color: var(--danger); }\
.temor-new-tab { display: flex; width: 100%; min-width: 0; }\
.temor-tab-box > *, .temor-new-tab > *, .temor-seg-slot > * { flex: 1 1 auto; min-width: 0; }\
.temor-split { display: flex; flex-direction: row; align-items: center; width: 100%; min-width: 0; gap: var(--s-2); }\
.temor-seg-slot { flex: 1 1 0; min-width: 0; display: flex; }\
.temor-seg { display: flex; align-items: center; gap: var(--s-6); min-width: 0; padding: 0 var(--s-2); color: var(--ink-soft); }\
.temor-seg[data-selected] { color: var(--ink); }\
.temor-seg-glyph { display: flex; flex: none; }\
.temor-seg-name { flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; }\
.temor-seg-slot[*|data-glyph=true] { flex: none; }\
.temor-seg-slot[*|data-glyph=true] .temor-seg-name { display: none; }\
.temor-seg-rule { flex: none; width: var(--hair); height: var(--s-14); background: var(--line); }\
.temor-host { display: flex; flex-direction: column; flex: 0 0 auto; gap: var(--s-4); }\
.temor-host-line { display: flex; align-items: center; gap: var(--s-8); min-width: 0; }\
.temor-pins { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--s-6); padding-bottom: var(--s-4); }\
.temor-pins > * { min-width: 0; }\
.temor-pin { position: relative; display: grid; place-items: center; height: 44px; border-radius: var(--r-tile); background: var(--f-pill-hover); color: var(--f-ink); }\
.temor-pin:hover, .temor-pin[*|data-selected=true] { background: var(--f-pill); }\
.temor-pin[*|data-selected=true] { box-shadow: var(--shadow-current); }\
.temor-pin[*|data-over=true] { box-shadow: 0 0 0 2px var(--accent-text); }\
.temor-pin-news { position: absolute; top: var(--s-2); right: var(--s-2); display: flex; }\
.temor-foot { flex: 1 1 auto; min-width: 0; }\
.temor-foot .temor-foot-lead, .temor-foot .temor-foot-end { display: flex; align-items: center; flex: 1000 1 0; min-width: 0; }\
.temor-foot-end { justify-content: flex-end; }\
.temor-foot-one { display: flex; align-items: center; min-width: 0; }\
.temor-head { display: flex; align-items: center; gap: var(--s-6); min-width: 0; }\
.temor-head-title { flex: 1 1 auto; min-width: 0; }\
.temor-head > .temor-dl-hold { flex: none; }\
.temor-dl-hold { position: relative; display: flex; }\
.temor-dl-dot { position: absolute; top: var(--s-3); right: var(--s-3); width: var(--s-6); height: var(--s-6); border-radius: var(--r-pill); background: var(--ok); pointer-events: none; }\
.temor-dl-dot.going { background: var(--ink-faint); }\
.temor-dl-list { display: flex; flex-direction: column; gap: var(--s-8); width: 320px; max-height: 420px; overflow-y: auto; padding: var(--s-8); }\
.temor-dl { display: flex; flex-direction: column; gap: var(--s-8); min-width: 0; }\
.temor-dl-actions { display: flex; gap: var(--s-8); }";
