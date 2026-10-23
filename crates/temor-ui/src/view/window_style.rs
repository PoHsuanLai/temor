//! The window's own sheet: the layout of the shell, the pane and the sidebar's rows. quire has no
//! column component at v0.3.1, and a `RawKeySurface` takes no style of its own. Colours and type
//! come from quire's tokens and the pane's palette, so this sheet has no design values. A tab's
//! selection is quire's source-list wash (da218b79, `--sel-neutral`), taken here through the selection tokens until
//! the quire pin reaches it: the sidebar says where you are, and the accent is left to the terminal's content.

pub const WINDOW_CSS: &str = "\
.temor-shell { position: relative; width: 100%; height: 100%; }\
.temor-edge-host { position: absolute; left: 0; top: 0; bottom: 0; width: 0; }\
.temor-content { flex: 1 1 auto; min-height: 0; min-width: 0; position: relative; }\
.temor-pane { position: relative; box-sizing: border-box; display: flex; flex-direction: column; width: 100%; height: 100%; padding: var(--s-8) var(--s-10); background: var(--term-bg); }\
.temor-edge { position: absolute; left: 0; right: 0; bottom: 0; display: flex; align-items: center; gap: var(--s-4); padding: 0 var(--s-4); pointer-events: none; }\
.temor-link-tip { position: absolute; width: var(--s-1); height: var(--s-16); pointer-events: none; }\
.temor-link-tip-spot { display: block; width: var(--s-1); height: var(--s-16); }\
.temor-sheet { background: var(--raise); border: var(--hair) solid var(--line); box-shadow: var(--shadow-sheet); }\
.temor-find { position: absolute; top: var(--s-4); right: var(--s-4); display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4); border-radius: var(--r-item); }\
.temor-find-field { width: calc(var(--s-16) * 12); }\
.temor-first-hint { position: absolute; pointer-events: none; white-space: nowrap; }\
.temor-keys { flex: 1 1 auto; min-height: 0; position: relative; overflow: hidden; }\
.temor-grid { position: relative; width: 100%; height: 100%; overflow: hidden; }\
.temor-grid-slice { position: absolute; left: 0; right: 0; pointer-events: none; }\
.temor-blist { position: absolute; top: 0; right: 0; bottom: 0; left: 0; display: flex; flex-direction: column; background: var(--term-bg); }\
.temor-blist-scroll { flex: 1 1 0; }\
.temor-blist-content { position: relative; }\
.temor-bseg { position: absolute; left: var(--s-8); right: var(--s-8); box-sizing: border-box; overflow: hidden; }\
.temor-blk-section { display: flex; align-items: flex-end; padding: 0 var(--s-8) var(--s-6); color: var(--term-ansi-12); font-size: var(--fs-small); white-space: nowrap; }\
.temor-blk-head { display: flex; align-items: center; gap: var(--s-8); padding: 0 var(--s-8) 0 var(--s-4); border-radius: var(--r-item) var(--r-item) 0 0; }\
.temor-blk-title { flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 600; font-size: var(--fs-reading); color: var(--term-fg); }\
.temor-blk-head[data-state=failed] .temor-blk-title { color: var(--term-ansi-1); }.temor-blk-head[data-state=running] .temor-blk-title { color: var(--term-ansi-3); }\
.temor-blk-fold { flex: none; display: flex; }.temor-blk-fold[data-none=true] { visibility: hidden; }\
.temor-blk-tools { flex: none; display: flex; align-items: center; gap: var(--s-4); }.temor-blk-more { display: flex; }\
.temor-blk-meta { flex: none; white-space: nowrap; font-size: var(--fs-small); color: var(--ink-faint); }.temor-blk-head[data-state=failed] .temor-blk-meta { color: var(--term-ansi-1); }\
.temor-blk-foot { border-radius: 0 0 var(--r-item) var(--r-item); }\
.temor-row-gap { position: absolute; left: 0; right: 0; pointer-events: none; background: var(--term-bg); }\
.temor-block-header { position: absolute; top: 0; left: 0; right: 0; box-sizing: border-box; display: flex; align-items: center; gap: var(--s-6); min-width: 0; overflow: hidden; background: var(--term-bg); border-bottom: var(--hair) solid var(--line-soft); }\
.temor-block-header-title { min-width: 0; }\
.temor-block-header[data-state=running] .temor-block-header-title { color: var(--term-ansi-3); }\
.temor-lens-layer { position: absolute; top: 0; left: 0; right: 0; bottom: 0; overflow: hidden; pointer-events: none; }\
.temor-lens { position: absolute; display: block; object-fit: contain; object-position: left top; }\
.temor-fact-chips { position: absolute; left: 0; right: 0; box-sizing: border-box; display: flex; align-items: center; justify-content: flex-end; gap: var(--s-6); pointer-events: none; overflow: hidden; }\
.temor-fact-chips[data-place=over] { justify-content: flex-start; background: var(--term-bg); }\
.temor-chip { display: flex; flex: none; align-items: center; gap: var(--s-5); box-sizing: border-box; height: calc(100% - var(--s-2)); max-height: var(--s-22); padding: 0 var(--s-6); border-radius: var(--r-micro); background: var(--fill-quaternary); border: var(--hair) solid var(--line-soft); color: var(--ink-soft); font-family: var(--font-code); font-size: var(--fs-small); white-space: nowrap; --chip-hue: var(--ink); }\
.temor-chip[data-kind=folder] { flex: 0 1 auto; min-width: 0; --chip-hue: var(--term-ansi-12); }\
.temor-chip-words { display: flex; gap: 0.6ch; min-width: 0; overflow: hidden; }.temor-chip-words > span { flex: none; }.temor-chip[data-kind=folder] .temor-chip-words > span { flex: 0 1 auto; min-width: 0; overflow: hidden; }\
.temor-chip-icon { display: flex; color: var(--chip-hue); }.temor-chip [data-tone=strong] { color: var(--chip-hue); font-weight: 600; }\
.temor-chip[data-kind=branch] { --chip-hue: var(--term-ansi-13); }.temor-chip[data-kind=changes] { --chip-hue: var(--ink-soft); }.temor-chip[data-kind=new] { --chip-hue: var(--term-ansi-3); color: var(--chip-hue); }\
.temor-chip[data-kind=python] { --chip-hue: var(--term-ansi-11); }.temor-chip[data-kind=ruby] { --chip-hue: var(--term-ansi-9); }.temor-chip[data-kind=go] { --chip-hue: var(--term-ansi-14); }.temor-chip[data-kind=node] { --chip-hue: var(--term-ansi-10); }.temor-chip [data-tone=added] { color: var(--ok); }.temor-chip [data-tone=removed] { color: var(--danger); }\
.temor-card { position: absolute; box-sizing: border-box; display: flex; flex-direction: column; justify-content: flex-start; padding: var(--s-3) var(--s-8) 0; overflow: hidden; pointer-events: none; border-radius: var(--r-item); background-color: var(--term-bg); }\
.temor-card-folder { position: absolute; box-sizing: border-box; display: flex; align-items: center; padding: 0 var(--s-8); overflow: hidden; white-space: nowrap; pointer-events: none; background: var(--term-bg); font-size: var(--fs-small); color: var(--term-ansi-12); }\
.temor-card-place { flex: none; font-size: var(--fs-small); color: var(--term-ansi-12); }\
.temor-card-body { display: flex; flex-direction: column; gap: var(--s-1); min-width: 0; line-height: 1.15; }\
.temor-card-head { display: flex; align-items: center; gap: var(--s-6); min-width: 0; }\
.temor-card-title { flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 700; font-size: var(--fs-reading); color: var(--term-fg); }\
.temor-card[data-state=running] .temor-card-title { color: var(--term-ansi-3); }\
.temor-card[data-state=failed] .temor-card-title { color: var(--term-ansi-1); }\
.temor-card-status { flex: none; font-size: var(--fs-small); color: var(--term-ansi-1); }\
.temor-card-note { min-width: 0; overflow: hidden; white-space: nowrap; font-size: var(--fs-small); color: var(--ink-soft); }\
.temor-card-note[data-tone=trouble] { color: var(--term-ansi-1); }\
.temor-card[data-tools=true] .temor-card-place, .temor-card[data-tools=true] .temor-card-glyph { visibility: hidden; }\
.temor-card-glyph { flex: none; display: grid; gap: var(--s-1); width: var(--s-8); }\
.temor-card-glyph i { display: block; height: var(--s-1); border-radius: var(--r-micro); background: var(--ink-faint); }\
.temor-card-bar { position: absolute; left: 0; bottom: 0; height: var(--s-2); background: var(--ink-faint); border-radius: 0 var(--r-micro) var(--r-micro) 0; }\
.temor-live-head { flex: none; display: flex; align-items: center; gap: var(--s-6); min-width: 0; padding: 0 0 var(--s-4); border-bottom: var(--hair) solid var(--line-soft); margin-bottom: var(--s-4); }\
.temor-live-title { flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 700; font-size: var(--fs-reading); color: var(--term-ansi-3); }\
.temor-live-head[data-state=ended] .temor-live-title { color: var(--ink); }\
.temor-live-status { flex: none; font-size: var(--fs-small); color: var(--ink-soft); }\
.temor-block-tools { position: absolute; display: flex; align-items: center; gap: var(--s-4); }.temor-block-more { display: flex; }\
.temor-keys:focus { outline: none; }\
.temor-peek { position: absolute; box-sizing: content-box; padding: var(--s-16); pointer-events: none; overflow: hidden; border-radius: var(--r-panel); }\
.temor-preedit { position: absolute; left: 0; bottom: 0; white-space: pre; text-decoration: underline; }\
.temor-quick { position: absolute; left: 8%; right: 8%; top: 4%; bottom: 24%; display: flex; flex-direction: column; border-radius: var(--r-panel); overflow: hidden; }\
.temor-quick-head { display: flex; align-items: center; justify-content: flex-end; gap: var(--s-8); padding: var(--s-8); }\
.temor-quick-body { flex: 1 1 auto; min-height: 0; position: relative; }\
.temor-side-box { width: 100%; height: 100%; }\
.temor-side { width: 100%; height: 100%; }\
.temor-tab-box { position: relative; display: flex; width: 100%; min-width: 0; }\
.temor-tab-box { --sel-bg: var(--sel-neutral); --sel-bg-quiet: var(--sel-neutral); --sel-ink: var(--ink); }\
.temor-tab-close { display: flex; flex: none; opacity: 0; }\
.temor-tab-box:hover .temor-tab-close, .temor-tab-close[*|data-selected=true] { opacity: 1; }\
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
