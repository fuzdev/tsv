# bom_prettier_divergence

tsv strips UTF-8 BOM (`0xEF 0xBB 0xBF`). Prettier preserves it.

tsv: strips BOM (parser skips, formatter never emits)
Prettier: preserves BOM if present

## Reason

BOM is meaningless for UTF-8 (legacy artifact from UTF-16 LE/BE detection). It causes problems with shebang scripts and some tools. Many modern formatters strip BOM (deno fmt, VS Code, etc.).

CSS and TypeScript BOM fixtures reference this README.
