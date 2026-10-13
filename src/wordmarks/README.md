# Legacy smooth wordmark assets

These masks are retained as reference assets for the previous smooth Aurora titles. Runtime titles now use native terminal pixel glyphs in `aurora.rs`; these masks are no longer loaded.

The platter.mask file is the antialiased grayscale coverage of the lowercase
app name in JetBrains Mono Regular: 132px with 4px letter spacing, on a
596×252 canvas, baseline y=186. The previous renderer averages coverage to the
terminal pixel dimensions, applies the selected theme colors, and encodes
Sixel. That bundled mask required no runtime font or image tools. Unsupported
terminals used plain text. JetBrains Mono uses the SIL OFL (see OFL.txt).
