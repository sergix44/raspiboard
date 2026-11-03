# Bundled fonts

## Monaspace Krypton

`MonaspaceKrypton-Regular-1.400.otf` is the unmodified Monaspace Krypton Regular
OpenType font from Monaspace v1.400, embedded for offline use. Its font family is
`Monaspace Krypton` and its style is Regular. All interface text uses it,
including sound-pad labels and crop inputs.

- [Project](https://github.com/githubnext/monaspace)
- [Pinned font](https://github.com/githubnext/monaspace/blob/v1.400/fonts/Static%20Fonts/Monaspace%20Krypton/MonaspaceKrypton-Regular.otf)
- [License](monaspace-LICENSE)

SHA-256: `eb8fcc2eb3fef3a8c1f5428e52379fc9badede6eb1f08c4f6d152fc496965e92`.

`src/ui/fonts.rs` defines the shared text font and embedded bytes.

## Lucide icons

`lucide-0.468.0.ttf` is the unmodified Lucide Static 0.468.0 TrueType font,
embedded in the application for offline use. Its font family is `lucide` and its
style is Regular. It is used for button icons and the crop editor's right-arrow
separator.

- [Project](https://lucide.dev/)
- [Pinned font](https://cdn.jsdelivr.net/npm/lucide-static@0.468.0/font/lucide.ttf)
- [Pinned glyph map](https://cdn.jsdelivr.net/npm/lucide-static@0.468.0/font/lucide.css)
- [License](lucide-LICENSE)

SHA-256: `0ff17e4ce16364d2dae72689336262960e7d5067cfcdb07ca283465a2a34af48`.

`src/ui/icons.rs` holds the glyph mappings. Keep the mappings and font version
together when updating the asset.
