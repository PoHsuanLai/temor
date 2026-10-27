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
.temor-keys { flex: 1 1 auto; min-height: 0; position: relative; overflow: hidden; }\
.temor-grid { position: relative; width: 100%; height: 100%; overflow: hidden; }\
.temor-blist { position: absolute; top: 0; right: 0; bottom: 0; left: 0; display: flex; flex-direction: column; background: var(--term-bg); }\
.temor-blist-scroll { flex: 1 1 0; }\
.temor-blist-content { position: relative; }\
.temor-bseg { position: absolute; left: var(--s-8); right: var(--s-8); box-sizing: border-box; overflow: hidden; }\
.temor-blk-head { display: flex; align-items: center; gap: var(--s-8); padding: 0 var(--s-8) 0 var(--s-4); border-radius: var(--r-item) var(--r-item) 0 0; }\
.temor-blk-title { flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 600; font-size: var(--fs-reading); color: var(--term-fg); }\
.temor-blk-head[data-state=failed] .temor-blk-title { color: var(--term-ansi-1); }.temor-blk-head[data-state=running] .temor-blk-title { color: var(--term-ansi-3); }\
.temor-blk-mark { flex: none; display: flex; color: var(--term-ansi-11); }.temor-blk-fold { flex: none; display: flex; }.temor-blk-fold[data-none=true] { visibility: hidden; }\
.temor-blk-tools { flex: none; display: flex; align-items: center; gap: var(--s-4); }.temor-blk-more { display: flex; }\
.temor-blk-meta { flex: none; white-space: nowrap; font-size: var(--fs-small); color: var(--ink-faint); }.temor-blk-head[data-state=failed] .temor-blk-meta { color: var(--term-ansi-1); }\
.temor-blk-sticky { position: absolute; top: 0; left: 0; right: 0; }.temor-blk-sticky .temor-blk-head { border-bottom: var(--hair) solid var(--line-soft); }\
.temor-blk-foot { border-radius: 0 0 var(--r-item) var(--r-item); }.temor-blk-head[data-whole=true] { border-radius: var(--r-item); }\
.temor-blk-composer { display: flex; flex-direction: column; justify-content: center; padding: 0 var(--s-12); border-radius: var(--r-item); }\
.temor-blk-composer[data-lines=many], .temor-blk-composer[data-choices=true] { justify-content: flex-start; padding-top: var(--s-8); }\
.temor-blk-line { display: flex; align-items: center; gap: var(--s-8); min-width: 0; }.temor-blk-composer[data-lines=many] .temor-blk-line { align-items: flex-start; }\
.temor-blk-choices { flex: none; display: flex; flex-direction: column; padding-bottom: var(--s-6); margin-bottom: var(--s-6); border-bottom: var(--hair) solid var(--line-soft); }\
.temor-blk-choice { flex: none; display: flex; align-items: center; padding: 0 var(--s-6); border-radius: var(--r-item); overflow: hidden; white-space: nowrap; color: var(--term-fg); font-size: var(--fs-reading); }\
.temor-blk-choice-note { margin-left: var(--s-8); color: var(--ink-faint); font-size: var(--fs-small); overflow: hidden; text-overflow: ellipsis; }\
.temor-blk-choice[data-picked=true] { background: var(--sel-neutral); }.temor-blk-choice[data-more=true] { color: var(--ink-faint); font-size: var(--fs-small); }\
.temor-blk-field { flex: 1 1 auto; min-width: 0; color: var(--term-fg); font-size: var(--fs-reading); }\
.temor-blk-img { position: absolute; top: 0; display: block; object-fit: contain; object-position: left top; }\
.temor-live-head { position: relative; z-index: var(--z-raise); flex: none; display: flex; align-items: center; gap: var(--s-6); min-width: 0; padding: 0 0 var(--s-4); border-bottom: var(--hair) solid var(--line-soft); margin-bottom: var(--s-4); }\
.temor-live-title { flex: 1 1 auto; min-width: 0; overflow: hidden; white-space: nowrap; font-weight: 700; font-size: var(--fs-reading); color: var(--term-ansi-3); }\
.temor-live-head[data-state=ended] .temor-live-title { color: var(--ink); }\
.temor-live-status { flex: none; font-size: var(--fs-small); color: var(--ink-soft); }\
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
.temor-tab-needs { display: flex; flex: none; }\
.temor-tab-box:hover .temor-tab-close, .temor-tab-close[*|data-selected=true] { opacity: 1; }\
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
.temor-favs { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--s-6); padding-bottom: var(--s-4); border-radius: var(--r-tile); }\
.temor-favs > * { min-width: 0; }\
.temor-favs[*|data-over=true] { box-shadow: 0 0 0 2px var(--accent-text); }\
.temor-favs-drop { grid-column: 1 / -1; display: grid; place-items: center; height: 40px; border-radius: var(--r-tile); border: var(--hair) dashed var(--f-line); color: var(--f-ink-soft); font-size: var(--fs-small); }\
.temor-fav { position: relative; display: grid; place-items: center; height: 44px; border-radius: var(--r-tile); background: var(--f-pill-hover); color: var(--f-ink); }\
.temor-fav:hover, .temor-fav[*|data-selected=true] { background: var(--f-pill); }\
.temor-fav[*|data-selected=true] { box-shadow: var(--shadow-current); }\
.temor-side-rule { height: var(--hair); margin: var(--s-4) var(--s-6); background: var(--f-line); }\
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
.temor-dl-actions { display: flex; gap: var(--s-8); }\
.temor-stage { display: flex; flex-direction: column; width: 100%; height: 100%; min-width: 0; min-height: 0; }\
.temor-page-bar { flex: none; display: flex; align-items: center; gap: var(--s-4); height: 32px; padding: 0 var(--s-6) 0 var(--s-8); border-bottom: var(--hair) solid var(--f-line); }\
.temor-page-title { flex: 1 1 auto; display: flex; align-items: baseline; gap: var(--s-6); min-width: 0; overflow: hidden; white-space: nowrap; }\
.temor-page-name { flex: none; color: var(--f-ink); font-weight: 600; }\
.temor-page-folder { min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--f-ink-soft); font-size: var(--fs-small); }\
.temor-page-tool { display: flex; border-radius: var(--r-tile); }\
.temor-page-tool[*|data-pressed=true] { background: var(--f-pill); }\
.temor-page { flex: 1 1 auto; min-height: 0; position: relative; }";
