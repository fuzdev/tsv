/**
 * Expected error detection - identifies parse errors that are expected because
 * the canonical parser (Svelte/acorn) also fails on the same input.
 *
 * These are NOT bugs in our parser - they represent genuinely unsupported syntax
 * (e.g., SCSS in <style>, CoffeeScript in <script>).
 */

/** A pattern that matches expected parse errors */
export interface ExpectedErrorPattern {
	/** Short identifier for this pattern */
	name: string;
	/** Human-readable reason this error is expected */
	reason: string;
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
 * Each pattern identifies file content that neither our parser nor the canonical
 * parser can handle. Add new patterns here as they're discovered.
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
];

/** Check if a parse error is expected (canonical parser also fails) */
export function checkExpectedError(content: string): ExpectedErrorResult {
	for (const pattern of EXPECTED_ERROR_PATTERNS) {
		if (pattern.matches(content)) {
			return { expected: true, pattern };
		}
	}
	return { expected: false };
}
