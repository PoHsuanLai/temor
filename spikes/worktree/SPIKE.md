# Spike: `git merge-tree --write-tree` as an overlap detector for agent worktrees

Scope: D19 / PLAN §8.2 "Early overlap warnings". Clones only (`repo-small` = temor, `repo-large` = quire at its
HEAD, 2510 tracked files, 1053 commits). Synthetic scenario repo in `synth/`. Scripts in `scripts/`.
Environment: git 2.55.0, GIT_CONFIG_GLOBAL=/dev/null, no writes to ~/temor, ~/quire, ~/.config, ~/.cache.

## 1. Git behaviour

- `git merge-tree --write-tree --name-only [--messages] A B`
  - exit 0: clean. Output: tree OID only.
  - exit 1: conflicts. Output: tree OID, conflicted paths (one per line), blank line, then messages (with --messages).
  - exit 1 ALSO for a bad ref (`fatal`-style message, first line not an OID). Exit 1 is therefore ambiguous.
    Rule: `git rev-parse --verify -q X^{commit}` first, and treat "first line is not a 40-hex OID" as error.
- `-z`: NUL-separated; the name list ends with an empty field. Use `-z` so odd paths are not quoted.
- Touches no worktree and no index: index SHA-256 and every file's mtime+size under the worktree were unchanged
  after 22 merge-tree runs over 11 agent worktrees (`integrity` in results.json).
- It does write loose objects (merged trees/blobs) into the clone's object store, no refs. First run: 68 -> 92
  loose objects, `git gc` cleans them.
- The snapshot step (below) uses `GIT_INDEX_FILE=<tmp>`; the real index SHA is unchanged after it (verified).

## 2. Scenarios (base B; agent A = `sc-x-1`, sibling S = `sc-x-2`, both from B)

| case | agent vs sibling (commits) | same-file signal | agent vs base tip | agent worktree snapshot vs sibling |
|---|---|---|---|---|
| a disjoint files | clean, none | none | clean, a.txt (base moved) | clean |
| b same file, different hunks | clean, a.txt | a.txt | clean | clean |
| c same line | **conflict** a.txt | a.txt | clean | **conflict** a.txt |
| d rename (A) vs edit (S) | clean, c.txt | **c.txt** (edit merged into c2.txt, checked) | clean | clean |
| e delete (A) vs edit (S) | **conflict** d.txt (modify/delete) | d.txt | clean | **conflict** d.txt |
| f binary both changed | **conflict** bin.dat | bin.dat | clean | **conflict** bin.dat |
| f2 binary one side | clean | none | clean | clean |
| g A commits line 2, edits line 15 uncommitted; S edits 15 | clean (**miss**) | a.txt | clean | **conflict** a.txt |
| g2 A has untracked new file; S commits same path | clean (**miss**) | none (untracked) | clean | **conflict** newfile.txt (add/add) |
| i identical change on both sides | clean | a.txt | clean | clean |
| h base moved ahead (A edits line 2, base edits line 20) | clean | none | clean, a.txt (base-side) | clean |

Notes:
- The "base tip" column is against `main`, which moved one commit (edits a.txt) after all branches were cut, so
  every a.txt row shows base-side movement. That is the intended "base moved ahead" signal, but it also means
  base-side `a.txt` appears in most rows. Real temor should show base movement separately from sibling overlap.
- Without `--no-renames`, `git diff --name-only` shows only the NEW name for a rename (`c2.txt`). The naive
  intersection missed case d; `--no-renames` (both old and new paths) catches it.

## 3. Signals and how to compute them

1. **Conflict** (blocking-level): merge-tree exit 1, paths from `--name-only`.
2. **Touches same file** (warning-level): `mb=$(git merge-base A X)`; intersect
   `git diff --no-renames --name-only mb A` with the same for X. Cheap (2-4 ms per pair here).
   Fires on b, d, g, i, and on every base-side overlap. False positives on b and i, see §4.
3. **Uncommitted work**: merge-tree only sees commits. Snapshot the worktree with a temp index:
   ```
   GIT_INDEX_FILE=$tmp git -C W read-tree HEAD
   GIT_INDEX_FILE=$tmp git -C W add -A
   T=$(GIT_INDEX_FILE=$tmp git -C W write-tree)
   C=$(git -C W -c user.name=temor -c user.email=temor@localhost commit-tree $T -p HEAD -m snapshot)
   rm -f $tmp
   ```
   Respects .gitignore, includes deletions and untracked files, leaves the real index and files untouched.
   Without it, cases g and g2 are missed (false negatives).

## 4. False positives / negatives vs what a human would call an overlap

- FP (warning only): b, different hunks in one file. A human says "same file, probably fine".
- FP (warning only): i, identical edit on both sides. merge-tree says clean; a human says harmless.
- FP: base-side commits that touched a file the agent also touched (h, and every a.txt row). Warning-level.
- FN fixed by the snapshot: g, g2 (uncommitted changes and untracked files).
- FN with `--name-only` only: renames (d), if the rename-aware diff is used.
- Not detected by any textual check: semantic conflicts (two agents change the same API in different files, or
  both add the same dependency to different manifests and the build breaks). Not tested beyond this list; state it
  in the UI as "text overlap only".
- Processes and ports are shared; the check is about files only, as PLAN §8.2 says.

## 5. Timing (median of 20 rounds, ms)

| check | small (temor, 5 files) | large (quire) |
|---|---|---|
| merge-tree, one pair, commits | 0.7 | 1.1 |
| merge-tree, agent vs base | 0.7 | 0.9 |
| snapshot of one dirty worktree (add -A + write-tree + commit-tree) | 14.3 | 64.8 |
| same-file signal, one pair (2 diffs + merge-base) | 2.2 | 4.4 |
| full round N=5 commits only (5 base + 10 pairs merge-tree, 10 same-file) | 36.4 | 40.7 |
| full round N=5 with 5 snapshots | 86.3 | 352.3 |

Each git call costs about 1-2 ms of process startup, so the commits-only round is dominated by spawn overhead.
The large-repo snapshot cost is the `add -A` scan of the worktree. merge-tree itself stays about 1 ms.
Timings used the 20-round median; min/max are in `timing-small.json` and `timing-large.json`.

## 6. Recommended algorithm for temor

Triggered when an agent command finishes (debounced), per agent worktree W, against base B and each sibling S:

1. Resolve refs once: `rev-parse --verify -q <ref>^{commit}` for B and every HEAD. Error out on failure.
2. Snapshot W with the temp-index method (§3.3) to commit SW. Its tree OID is the cache key for W's state.
3. For each X in {B, siblings}, cache by (tree(SW), X):
   a. `git merge-tree --write-tree --name-only -z SW X`. Exit 0 = clean; exit 1 with an OID line = conflict paths;
      anything else = error, shown on the tab.
   b. Same-file: merge-base, then `diff --no-renames --name-only` on both sides, intersect.
4. Base only: `git rev-list --left-right --count B...SW` for ahead/behind, and base-side files since merge-base.
5. Tab shows, in this order:
   - "Conflicts with <tab> in <file>" (red): merge-tree conflict paths.
   - "Both touch <file>" (amber): same-file signal, minus files already listed as conflicts.
   - "Base moved: N files, M overlap yours" (amber/grey).
   - Ahead/behind and files changed (plain facts).
   Never show a severity above these two.
6. Cost budget: about 40 ms (small) and 350 ms (large) per round for N=5, off the render path. Re-run only the
   pairs whose tree OIDs changed, which keeps steady-state cost near the snapshot cost for the one agent that
   changed.

Keep it deterministic: no per-tool rules, no content heuristics, no AI. The only judgement is the two-level
display, and the team can tune that later without changing the algorithm.

## Known gaps

- Textual only: semantic conflicts are invisible (§4).
- Snapshot cost grows with worktree size (65 ms on quire); a `git status --porcelain` short-circuit for the
  clean case would help but was not measured.
- Only the synthetic repo has a moved base in the scenario table; timing used an unmoved base.
- Not tested: LFS, submodules, sparse checkouts, merge-tree on a non-linux host.
