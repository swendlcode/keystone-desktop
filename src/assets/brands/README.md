# Export target marks

Drop the three logos in here and they replace the lettered badges in
Settings automatically. No code change — `TargetMark` picks up whatever is
present at build time.

| File              | Product   | Where to get it |
|-------------------|-----------|-----------------|
| `rekordbox.svg`   | rekordbox | Pioneer DJ press / brand resources |
| `traktor.svg`     | Traktor   | Native Instruments press resources |
| `serato.svg`      | Serato    | Serato brand assets |

`.png` works too; SVG scales better at 20px.

These are other companies' trademarks. They are used here only to name the
product each row exports to, which is what the marks are for — but the
assets are theirs, so they are not committed to this repository and the
fallback has to keep working without them. Anyone building Keystone from a
clean checkout sees the lettered badges, and that is a complete build.
