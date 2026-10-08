# Follow-ups

Review leftovers that didn't block a merge. Each one names the lane it came from and the PLAN section.

## temor-core (lane/core, reviewed at 03031e9)
- [ ] `TrialId` is only as unique as `TurnRef`: every Ask must get a fresh `TurnRef` (document it, or
      add a counter). §8.3
- [ ] `Origin::LocalStream` emits `Remember(Grant)`, but no consent store exists for local panes
      (`HostConsent` is per host). Add one at the edge, or always ask. §9
- [ ] A typed proposal deleted by hand and replaced by the person's own command still logs
      `Ran{Edited}`. Tell "edited" apart from "replaced". §8.3
- [ ] U+2800 (braille blank) looks like a space but isn't default-ignorable. Consider treating it as
      a space. §8.3
