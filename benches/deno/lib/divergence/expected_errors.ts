/**
 * Expected error detection - identifies parse errors that are expected because
 * the input is not standard, tsv-supported syntax.
 *
 * Origins:
 *  - **Canonical-parser-fails** (Svelte/acorn also reject): SCSS in `<style>`,
 *    CoffeeScript in `<script>`.
 *  - **Non-standard CSS** (SCSS/LESS/PostCSS/CSS-Modules/front-matter/IE hacks):
 *    tsv targets standard CSS via Svelte's `parseCss` and strictly rejects these.
 *    Prettier/oxfmt/biome only "pass" them because they run lenient PostCSS-family
 *    parsers — so tsv rejecting them is correct, not a bug.
 *  - **Non-standard JS/TS proposals** (stage-1/2/3): pipeline (`|>`), source-phase
 *    imports — tsv targets standard ECMAScript/TypeScript; prettier formats them
 *    via Babel plugins. Markers are the proposal's unique operator/keyword
 *    sequences (see the JS/TS section for the discipline, incl. why bind `::` is
 *    deliberately left unmatched).
 *  - **Test-harness artifacts** (all languages): the `<<<PRETTIER_RANGE_*>>>`
 *    range-format markers, syntactically invalid in every language.
 *
 * These are NOT bugs in our parser. Genuine standard-language parser gaps (empty
 * custom-property values, `url()` special chars, tagged-template invalid escapes,
 * etc.) are deliberately NOT matched here so they keep surfacing as `error`s.
 *
 * Safety gate: `check_expected_error` runs ONLY on a file that already failed to
 * parse (the `catch` branch in `corpus_compare_format.ts`), so a clean file whose source
 * merely contains a marker in a string/comment never reaches these patterns.
 */

import type { Language } from '../types.ts';

/** A pattern that matches expected parse errors */
export interface ExpectedErrorPattern {
	/** Short identifier for this pattern */
	name: string;
	/** Human-readable reason this error is expected */
	reason: string;
	/**
	 * Languages this pattern applies to. Omit to apply to every language.
	 * CSS patterns MUST scope to `['css']` — a marker like `prop: {` would
	 * otherwise misclassify a genuinely-broken TypeScript object type.
	 */
	languages?: Language[];
	/** Check if a file's content matches this expected error pattern */
	matches: (content: string) => boolean;
}

/** Result of checking a parse error against expected patterns */
export interface ExpectedErrorResult {
	/** Whether the error matches an expected pattern */
	expected: boolean;
	/** The matching pattern, if any */
	pattern?: ExpectedErrorPattern;
}

/**
 * Known expected error patterns.
 *
 * Each pattern identifies file content that is not standard, tsv-supported
 * syntax. Add new patterns here as they're discovered.
 *
 * CSS pattern discipline: sniff on **content
 * markers that are unambiguously non-standard**, never on directory names —
 * a future real standard-CSS gap must not hide behind a path. Markers are
 * anchored tightly (e.g. SCSS `$x:` declarations, `#{}`/`$()` interpolation,
 * non-standard at-rules, front matter) so they cannot fire on the genuine
 * gaps tracked in Bucket A.
 */
export const EXPECTED_ERROR_PATTERNS: ExpectedErrorPattern[] = [
	{
		name: 'scss_style',
		reason: "SCSS in <style> — Svelte's CSS parser doesn't support SCSS",
		matches: (content) =>
			/<style[^>]*(?:lang\s*=\s*["']scss["']|type\s*=\s*["']text\/scss["'])[^>]*>/.test(content),
	},
	{
		name: 'unsupported_script_lang',
		reason: "Non-JS/TS script language — Svelte's JS parser doesn't support it",
		matches: (content) =>
			/<script[^>]*lang\s*=\s*["'](?:coffee|coffeescript|pug|jade)["'][^>]*>/.test(content),
	},

	// --- Non-standard CSS (Bucket B) — tsv correctly rejects, prettier/oxfmt/biome
	// only pass via lenient PostCSS-family parsers. Scoped to CSS so the markers
	// can't fire on broken TS/Svelte. ---
	{
		name: 'css_front_matter',
		reason: 'YAML/front-matter header (`---`) — a PostCSS/prettier feature, not CSS',
		languages: ['css'],
		matches: (content) => /^---\r?\n/.test(content),
	},
	{
		name: 'scss_variable',
		reason: 'SCSS `$variable` declaration — not standard CSS',
		languages: ['css'],
		// `$ident:` declaration; excludes the standard attr-suffix selector `[a$=b]`
		// (there `$` is followed by `=`, not an identifier).
		matches: (content) => /(?:^|[\s;{])\$[\w-]+\s*:/m.test(content),
	},
	{
		name: 'scss_interpolation',
		reason: 'SCSS/LESS interpolation (`#{}` / `$()`) — not standard CSS',
		languages: ['css'],
		matches: (content) => /[#$]\{|\$\(/.test(content),
	},
	{
		name: 'scss_less_at_rule',
		reason: 'SCSS/LESS/PostCSS at-rule (@mixin/@if/@extend/@nest/@value/…) — not standard CSS',
		languages: ['css'],
		matches: (content) =>
			/@(?:mixin|include|if|else|each|for|while|function|return|extend|use|forward|content|nest|value)\b/i
				.test(content),
	},
	{
		name: 'less_import',
		reason: 'LESS `@import … .less` — not standard CSS',
		languages: ['css'],
		matches: (content) => /@import[^;{]*\.less\b/i.test(content),
	},
	{
		name: 'css_scss_nested_props',
		reason: 'SCSS nested properties (`font: { … }`) — not standard CSS',
		languages: ['css'],
		// A declaration-like line ending in `{` (`font: {`). Standard CSS never
		// has `ident: {`; custom props (`--foo: {`) start with `-`, so the
		// `[a-z]`-anchored class leaves the deferred top-level-block gap as an error.
		matches: (content) => /^\s*[a-z][a-z-]*\s*:\s*\{\s*$/m.test(content),
	},
	{
		name: 'css_ie_hacks',
		reason: 'IE hacks (`*zoom`, `_width`, `\\9`) — not standard CSS',
		languages: ['css'],
		matches: (content) => /(?:[;{]\s*[*_+][a-z-]+\s*:)|[\w)]\\9\b/i.test(content),
	},

	// --- Test-harness artifact (all languages). The range-format API embeds
	// literal markers in the source, syntactically invalid in every language —
	// same class as the `cursor/` / `multiparser*` corpus exclusions, but
	// classified here rather than excluded (the CSS suite already surfaced these,
	// and `expected_errors` keeps them visibly counted). ---
	{
		name: 'range_marker',
		reason: 'Prettier range-format marker (`<<<PRETTIER_RANGE_*>>>`) — a test harness artifact',
		matches: (content) => /<<<PRETTIER_RANGE_(?:START|END)>>>/.test(content),
	},

	// --- Non-standard JS/TS proposals (stage-1/2/3). tsv targets standard
	// ECMAScript/TypeScript and rejects these; prettier formats them via Babel
	// plugins. Scoped to `typescript` (the .js+.ts bucket). The markers are the
	// proposal's unique operator/keyword sequences — they can appear in standard
	// source only inside strings/regexes/comments, and the classifier runs only
	// after a parse failure, so a clean file with e.g. `tokio::spawn` in a string
	// never reaches here. The bind operator `::` is deliberately NOT matched: it
	// is indistinguishable from embedded Rust/SQL/IPv6 strings (`a::b`), which are
	// common in this corpus — too large a collision surface to mark safely without
	// risking a hidden genuine gap. ---
	{
		name: 'js_pipeline_operator',
		reason: 'Pipeline operator (`|>`) — a stage-2 proposal, not standard ECMAScript',
		languages: ['typescript'],
		// Operand `|>` operand — excludes the `|`/`>` regex-alternation forms
		// (`/>|>(`, `(\+|~|>|\|\|)`) that appear in real parser source.
		matches: (content) => /[)\]\w$%]\s*\|>\s*[\w$%(]/.test(content),
	},
	{
		name: 'js_source_phase_import',
		reason:
			'Source-phase import (`import source x from …` / `import.source(…)`) — a stage-3 proposal',
		languages: ['typescript'],
		// `import.source(` (expression form) or `import source <binding> … from`
		// (declaration form). The declaration branch requires a binding token AND a
		// `from`, so the standard default import `import source from "x"` (a binding
		// named `source`) is left as-is.
		matches: (content) =>
			/\bimport\.source\s*\(|\bimport\s+source\s+(?:[\w$]+|\*|\{)[^;\n]*\bfrom\b/.test(content),
	},
];

/**
 * Check if a parse error is expected.
 *
 * @param content  the file's source
 * @param language the file's language; patterns with a `languages` list only
 *                 apply when it includes this value (untagged patterns apply to all)
 */
export function check_expected_error(
	content: string,
	language?: Language,
): ExpectedErrorResult {
	for (const pattern of EXPECTED_ERROR_PATTERNS) {
		if (pattern.languages && (!language || !pattern.languages.includes(language))) {
			continue;
		}
		if (pattern.matches(content)) {
			return { expected: true, pattern };
		}
	}
	return { expected: false };
}
