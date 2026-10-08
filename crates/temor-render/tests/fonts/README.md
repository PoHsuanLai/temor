# Test fonts

Fixed fonts for the golden images, so renders never depend on the machine's fonts. All are
under the SIL Open Font License 1.1 (`OFL.txt`); none has a Reserved Font Name, so the subsets
keep their names. Subsetted with fontTools `pyftsubset` (all layout features kept) from Fedora 44's
packages:

| File | Source | Kept |
|---|---|---|
| `NotoSansMono-Regular-subset.ttf`, `NotoSansMono-Bold-subset.ttf` | google-noto-sans-mono-fonts 20251201 | Basic Latin, Latin-1, general punctuation, € |
| `NotoSansMonoCJK-subset.otf` | google-noto-sans-mono-cjk-vf-fonts 2.004 (JP, instanced at wght 400) | 日本語中文漢字表示幅 |
| `NotoColorEmoji-subset.ttf` | google-noto-color-emoji-fonts 20260921 (`Noto-COLRv1.ttf`, COLRv1 only) | 😀 👍 skin tones, 👨 👩 👧 with ZWJ, ❤️, 🎉, 🚀 |
