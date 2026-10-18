/**
 * The options reader and the format API every tsv npm package exports — the shared half
 * of one hand-written facade over whichever engine the package carries; the parse
 * family is `api_parse.js` and the type strip `api_strip.js`, which build on this file.
 *
 * The engines are flat: the WASM bindings (`@fuzdev/tsv-wasm`, `-parse-wasm`,
 * `-format-wasm`) and the native N-API addon (`@fuzdev/tsv`) each export
 * `format_<lang>(source, source_type?)` and `parse_<lang>_json(source, source_type?)`
 * (the WASM ones `parse_<lang>(source, source_type?)` too). The facade turns that into
 * the published surface — `(source, options?)` with an acorn-style bag — and is the ONE
 * options reader: staged verbatim into every package (`scripts/patch_npm_package.ts`,
 * `scripts/build_napi_packages.ts`), so the error a caller matches on is the same text
 * whichever package they installed. The texts are the facade's own: the raw engines refuse
 * a bad source type in their own words (`tsv_arena`'s, shared by the WASM and N-API
 * decoders), and the facade never lets one reach them.
 *
 * The bag: `parse_<lang>(source, {locations?, sourceType?})`,
 * `parse_<lang>_json(source, {sourceType?})`, `format_<lang>(source, {sourceType?})`, and
 * `strip_<lang>(source, {sourceType?})`.
 * `sourceType` (`'script'` / `'module'`) is TypeScript's alone — Svelte hard-wires
 * `module` and CSS has no goal — so the other languages refuse a set one. Unknown keys
 * error whatever their value (a typo like `{locatons: true}` must not silently hand back
 * the default); a supported key explicitly set to `undefined` means its default,
 * including `sourceType` on a language that refuses a set one — which is what lets one
 * bag forward to whichever export (`npm/cli.js` does). `parse_*_json` and `format_*`
 * take no `locations`: the JSON string is the wire, and `loc` is a view over objects; a
 * format emits no wire at all. A `_json` export refuses the key with that explanation, a
 * format export as an unknown key.
 *
 * Every argument refusal — the bag's and the source's — is a `TypeError`, thrown.
 *
 * Every parse, parse-JSON and format export RETURNS what it concludes about the source, as
 * the strip does (`api_strip.js`): `{ok: true, …}` with the product (`ast`, `json` or `code`),
 * or `{ok: false, error}` for a source it cannot handle. `error` is a plain object — no stack
 * captured per failure, and structured-clone-able, so a worker can post it — whose span is
 * shaped like a wire node's: `start` and `end` the UTF-16 offsets a wire node there would
 * carry, `loc` their `{start: {line, column}, end: {line, column}}` under the document's line
 * rule, so `create_locator(source, {language}).loc_of(error)` (`locations.js`) deep-equals
 * `error.loc`. `message` says what went wrong, on one line; `frame` is the excerpt under it —
 * the `line:col` header (`loc.start.line:loc.start.column + 1`) with the source line, then
 * the caret line — so `${message}\n${frame}` is the text the native CLI prints:
 *
 * - `{kind: 'parse', message, frame, start, end, loc}` — the source does not parse. A parse
 *   error knows a point, not a span, so `end === start`.
 * - `{kind: 'refused', message, frame, start, end, loc}` — a format's refusal of a source that
 *   parses: a lone CR inside a Svelte in-tag `//` comment, which the format's CR fold would
 *   end early; the span is the CR.
 *
 * The engines build that result themselves and RETURN it in the product's place — a string
 * for every export but the WASM engine's object `parse_<lang>`, which returns `{ok: true, ast}`
 * itself — so the facade tells the two apart by `typeof` (or by `ok`) and copies nothing;
 * `read_failure` only checks the shape, and throws a malformed one as the engine bug it is
 * (as it does a `kind` the export never returns: a refusal from a parse export). What still
 * THROWS is an engine failure with no point, passed through as itself: a source over the
 * size cap (a plain `Error`), a caught panic, a WASM trap (`RuntimeError`) or stack
 * exhaustion (`RangeError`). The facade's own checks leave an engine no argument to refuse.
 *
 * A `source` that is not a string is refused before the engine sees it
 * (`read_source`), since the engines would each answer it differently.
 *
 * An unset source type stays unset all the way to the engine, which answers it per
 * family: a parse reads it as `module` (its wire's `Program.sourceType` is a claim one
 * settled grammar has to produce), a format or a strip as none named — the module grammar,
 * retried as a script — so a legacy sloppy script formats through
 * `format_typescript(source)`, and strips through `strip_typescript(source)`, with no bag at
 * all.
 *
 * Why two files: the parse half reconstructs `loc` with `locations.js`, and the
 * format-only package (`@fuzdev/tsv-format-wasm`) ships neither — so this file imports
 * nothing, and that package loads nothing it has no use for. Each entry (`index.js`,
 * `browser.js`, the napi loader) is a thin adapter: it hands `create_format_api` /
 * `create_parse_api` its engine's functions and re-exports the result by name.
 * `parse_internal_*` (bench-only) never passes through here.
 */

/**
 * The export kinds a bag is read for: `parse` (`parse_<lang>`, which takes
 * `locations`), `parse_json` (`parse_<lang>_json`, which refuses it), `format`, and
 * `strip` (`strip_<lang>`).
 *
 * @typedef {'parse' | 'parse_json' | 'format' | 'strip'} ExportKind
 */

/**
 * Read an options bag against one export's key set, returning
 * `{locations, source_type}` (`locations` default `false`, `source_type` default
 * `undefined` — unset, not `'module'`; see the module doc).
 *
 * The key set follows from the export: `locations` is a key of a `parse` export alone (a
 * `parse_json` export refuses it with its own explanation — the export returns the wire,
 * which `loc` is not part of — and a `format` export as an unknown key), and `sourceType`
 * is settable for TypeScript alone, the one language with a parse goal. Every error names
 * the export family, `parse`, `format` or `strip`.
 *
 * `undefined` / `null` mean all defaults; a string, a number or an array (which is
 * `typeof 'object'` and has no keys, so it would otherwise read as all defaults) is
 * refused. Any other object is read by its own enumerable keys and nothing else, so a
 * non-plain one — a `Date`, a `Map` — is not refused: it reads as a keyless bag, i.e.
 * all defaults. A key's getter runs once, and one that throws surfaces as
 * `failed to read <noun> option '<name>'` with the getter's error as its `cause`.
 *
 * @param {unknown} options - the caller's bag
 * @param {ExportKind} kind - the export the bag is for
 * @param {string} language - the export's language, as the engine names it
 * @returns {{locations: boolean, source_type: 'script' | 'module' | undefined}}
 * @throws TypeError when `options` is not an object, carries a key the export does not take,
 *   holds a value of the wrong type, or has a getter that throws
 */
export function read_options(options, kind, language) {
	const noun = kind === 'format' || kind === 'strip' ? kind : 'parse';
	const takes_locations = kind === 'parse';
	const takes_source_type = language === 'typescript';
	const parsed = { locations: false, source_type: undefined };
	if (options === undefined || options === null) return parsed;
	if (typeof options !== 'object' || Array.isArray(options)) {
		throw new TypeError(`${noun} options must be an object`);
	}
	for (const name of Object.keys(options)) {
		if (name === 'locations' && takes_locations) {
			const value = read_option(options, name, noun);
			if (value === undefined) continue;
			if (typeof value !== 'boolean') {
				throw new TypeError(
					`${noun} option 'locations' must be a boolean (got ${describe_value(value)})`
				);
			}
			parsed.locations = value;
		} else if (name === 'locations' && kind === 'parse_json') {
			throw new TypeError(
				`${noun} option 'locations' is not supported by parse_${language}_json — the ` +
					`JSON string is the span-only wire; use parse_${language}(source, {locations: true})`
			);
		} else if (name === 'sourceType') {
			const value = read_option(options, name, noun);
			// checked before the language's refusal: a forwarded bag spells the
			// inapplicable source type `undefined`, and that must pass everywhere
			if (value === undefined) continue;
			if (!takes_source_type) {
				throw new TypeError(`${noun} option 'sourceType' is only supported for TypeScript`);
			}
			if (value !== 'script' && value !== 'module') {
				throw new TypeError(
					`${noun} option 'sourceType' must be 'script' or 'module' (got ${describe_value(value)})`
				);
			}
			parsed.source_type = value;
		} else {
			const detail =
				takes_locations && takes_source_type
					? "expected 'locations' or 'sourceType'"
					: takes_locations
						? "expected 'locations'"
						: takes_source_type
							? "expected 'sourceType'"
							: 'this export takes no options';
			throw new TypeError(`unknown ${noun} option ${quote_string(name)} (${detail})`);
		}
	}
	return parsed;
}

/**
 * A refused value as an error message's `(got …)` names it: a string quoted, a number or
 * boolean as written, `null`, `none` for `undefined`, and anything else by its `typeof`.
 * A string is echoed as a single-quoted literal with JSON's escapes plus the terminal-unsafe
 * characters JSON leaves raw (so a line break, a control, a bidi control or a lone surrogate
 * never reaches the message raw) and clipped past
 * `DESCRIBED_STRING_MAX` UTF-16 units with a `…` inside the quotes. `locations.js` restates
 * it, since that module imports nothing.
 *
 * @param {unknown} value - the refused value
 * @returns {string}
 */
function describe_value(value) {
	if (typeof value === 'string') return quote_string(value);
	if (typeof value === 'number' || typeof value === 'boolean') return String(value);
	if (value === undefined) return 'none';
	return value === null ? 'null' : typeof value;
}

/** The longest refused string `describe_value` echoes whole, in UTF-16 units. */
const DESCRIBED_STRING_MAX = 40;

/**
 * `value` as a single-quoted literal, escaped as `JSON.stringify` escapes (with `'` escaped
 * and `"` not, for the other quote) plus `\uXXXX` for what JSON leaves raw but a terminal
 * reads as a line break, a control or a reordering (`TERMINAL_UNSAFE`), clipped to
 * `DESCRIBED_STRING_MAX` units plus `…` — one unit sooner where the cut would split a
 * surrogate pair.
 *
 * @param {string} value
 * @returns {string}
 */
function quote_string(value) {
	let cut = value.length;
	if (cut > DESCRIBED_STRING_MAX) {
		cut = DESCRIBED_STRING_MAX;
		const last = value.charCodeAt(cut - 1);
		if (last >= 0xd800 && last <= 0xdbff) cut--;
	}
	const body = JSON.stringify(value.slice(0, cut))
		.slice(1, -1)
		.replace(/\\"/g, '"')
		.replace(/'/g, "\\'")
		.replace(TERMINAL_UNSAFE, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`);
	return `'${body}${cut < value.length ? '…' : ''}'`;
}

/** The characters JSON leaves raw that a terminal reads as a line break (NEL, LS, PS), a
 * C1 control or DEL, or a bidi control. */
const TERMINAL_UNSAFE = /[\u007f-\u009f\u061c\u2028\u2029\u200e\u200f\u202a-\u202e\u2066-\u2069]/g;

/**
 * Read one supported key off a bag, naming the option when its getter throws — the
 * caller's own error rides along as the `cause`. An unknown key is refused before it
 * is read, so its getter never runs.
 *
 * @param {object} options - the caller's bag
 * @param {string} name - a key this export takes
 * @param {'parse' | 'format' | 'strip'} noun - the export family, as every error names it
 * @returns {unknown}
 * @throws TypeError when the key's getter throws
 */
function read_option(options, name, noun) {
	try {
		return /** @type {Record<string, unknown>} */ (options)[name];
	} catch (cause) {
		throw new TypeError(`failed to read ${noun} option '${name}'`, { cause });
	}
}

/**
 * Refuse a `source` that is not a string, with one message on every engine. Without
 * it the engines disagree: wasm-bindgen's string glue hands the value to
 * `TextEncoder.encodeInto`, which stringifies it on Deno, Bun and browsers (so `42`
 * parses as the empty document) and throws a Node-specific `TypeError` on Node, while
 * the N-API addon throws its own conversion error.
 *
 * A string that is not well-formed UTF-16 — one holding a lone surrogate — is refused too:
 * both engines read the source as UTF-8, and the conversion replaces a lone surrogate with
 * U+FFFD, so a format would silently hand back a different string and a parse would report
 * a value and offsets for text the caller never wrote.
 *
 * @param {unknown} source - the caller's source
 * @param {'parse' | 'format' | 'strip'} noun - the export family, as every error names it
 * @returns {string} the same `source`
 * @throws TypeError when `source` is not a string, or holds a lone surrogate
 */
export function read_source(source, noun) {
	if (typeof source !== 'string') {
		throw new TypeError(
			`${noun} source must be a string (got ${source === null ? 'null' : typeof source})`
		);
	}
	const lone = lone_surrogate_offset(source);
	if (lone !== -1) {
		throw new TypeError(
			`${noun} source must be well-formed UTF-16 (a lone surrogate at offset ${lone})`
		);
	}
	return source;
}

/**
 * The UTF-16 offset of the first lone surrogate in `source`, or `-1` when it is well-formed.
 * `String.prototype.isWellFormed` answers the common case natively where the runtime has it
 * (Node 20+, Deno, Bun, current browsers); the scan runs only to place a lone surrogate, or
 * on a runtime without it.
 *
 * @param {string} source
 * @returns {number}
 */
function lone_surrogate_offset(source) {
	if (typeof source.isWellFormed === 'function' && source.isWellFormed()) return -1;
	for (let i = 0; i < source.length; i++) {
		const unit = source.charCodeAt(i);
		if (unit < 0xd800 || unit > 0xdfff) continue;
		if (unit <= 0xdbff) {
			const next = source.charCodeAt(i + 1);
			if (next >= 0xdc00 && next <= 0xdfff) {
				i++; // a pair
				continue;
			}
		}
		return i;
	}
	return -1;
}

/** The error kinds a parse export returns. */
export const PARSE_KINDS = new Set(['parse']);

/** The error kinds a format export returns: a parse failure, or a refusal of a source that
 * parses. */
const FORMAT_KINDS = new Set(['parse', 'refused']);

/**
 * An engine's returned failure, checked: the `{ok: false, error}` the engine built, handed
 * back as it is when its `error` is the shape every failure takes — a `kind` this export
 * returns (`kinds`), a string `message` and `frame`, integer `start` and `end`, and a `loc`
 * whose `start` and `end` each hold an integer `line` and `column` — and thrown otherwise, since only an engine bug returns anything else in its
 * product's place.
 *
 * @param {unknown} value - what the engine returned in place of its product
 * @param {ReadonlySet<string>} kinds - the error kinds this export returns
 * @returns {{ok: false, error: Record<string, any>}}
 * @throws Error when `value` is no such failure
 */
export function read_failure(value, kinds) {
	if (typeof value === 'object' && value !== null) {
		const { ok, error } = /** @type {Record<string, any>} */ (value);
		if (
			ok === false &&
			typeof error === 'object' &&
			error !== null &&
			kinds.has(error.kind) &&
			typeof error.message === 'string' &&
			typeof error.frame === 'string' &&
			Number.isInteger(error.start) &&
			Number.isInteger(error.end) &&
			is_position(error.loc?.start) &&
			is_position(error.loc?.end)
		) {
			return /** @type {{ok: false, error: Record<string, any>}} */ (value);
		}
	}
	throw new Error('internal error: the engine returned neither its product nor a failure', {
		cause: value
	});
}

/**
 * Whether `value` is a `{line, column}` position of integers.
 *
 * @param {unknown} value
 * @returns {boolean}
 */
function is_position(value) {
	if (typeof value !== 'object' || value === null) return false;
	const { line, column } = /** @type {Record<string, unknown>} */ (value);
	return Number.isInteger(line) && Number.isInteger(column);
}

/**
 * Build the format family over one engine's `format_<lang>(source, source_type?)` functions,
 * keyed by language. Each engine function returns the formatted string, or the
 * `{ok: false, error}` it built for a parse failure or a refusal; each published function
 * returns `{ok: true, code}` or that failure (`read_failure`).
 *
 * Each function is defined under a computed key, which names it: `format_css.name` is
 * `'format_css'`, as a caller's stack trace and a `function.name` check expect of an export.
 *
 * @param {Record<string, (source: string, source_type?: string) => unknown>} format
 * @returns {Record<string, (source: unknown, options?: unknown) => any>} `format_<lang>`
 */
export function create_format_api(format) {
	/** @type {Record<string, (source: unknown, options?: unknown) => any>} */
	const api = {};
	for (const [language, format_language] of Object.entries(format)) {
		Object.assign(api, {
			[`format_${language}`]: (source, options) => {
				const text = read_source(source, 'format');
				const { source_type } = read_options(options, 'format', language);
				const code = format_language(text, source_type);
				if (typeof code === 'string') return { ok: true, code };
				return read_failure(code, FORMAT_KINDS);
			}
		});
	}
	return api;
}
