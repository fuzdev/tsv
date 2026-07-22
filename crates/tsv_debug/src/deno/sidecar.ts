// Deno Sidecar for tsv_debug
// Long-running process for JS tools. Communicates via JSON-lines over stdio.

// ⚠ SYNC — each canonical version below is pinned in THREE places that must stay
// identical: (1) this VERSIONS object, (2) the static `import` specifiers just below
// (a static import can't interpolate a const, so the literal is repeated by necessity),
// and (3) benches/js/package.json (read by benches/js/lib/versions.ts).
// These can't be DRYed: the release binary embeds this file as a string and runs it
// WITHOUT that package.json, so it can't read the bench's pins at runtime.
// Agreement across all the pin sites (these three + actor.rs's acorn import-map
// pin) is enforced by `deno task pins:audit` (scripts/check_canonical_pins.ts,
// gated in `deno task check`).
//
// Bumping prettier / svelte / acorn / @sveltejs/acorn-typescript / prettier-plugin-svelte
// is NOT a routine refresh — it re-baselines the entire fixture corpus (these tools define
// every fixture's expected.json + output_prettier.*). After any bump: run
// `deno task fixtures:update` and review the churn. See benches/js/CLAUDE.md
// §"Canonical baseline is coupled" for the full procedure.
//
// NOTE: Requires deno.json with "acorn": "npm:acorn@8.16.0" import map
// to ensure @sveltejs/acorn-typescript uses the same acorn instance.
const VERSIONS = {
	prettier: '3.9.5',
	'prettier-plugin-svelte': '4.1.1',
	svelte: '5.56.4',
	acorn: '8.16.0',
	'@sveltejs/acorn-typescript': '1.0.11',
} as const;

// TODO verify there's not a better solution to use deno.json here, see the above NOTE too
// Imports are like this because these don't have the deno.json when used by the release binary.
// deno-lint-ignore no-import-prefix
import * as prettier from 'npm:prettier@3.9.5';
// deno-lint-ignore no-import-prefix
import prettierPluginSvelte from 'npm:prettier-plugin-svelte@4.1.1';
// deno-lint-ignore no-import-prefix
import { compile as svelteCompile, parse as svelteParse, parseCss } from 'npm:svelte@5.56.4/compiler';
// deno-lint-ignore no-import-prefix
import * as acorn from 'npm:acorn@8.16.0';
// deno-lint-ignore no-import-prefix
import { tsPlugin } from 'npm:@sveltejs/acorn-typescript@1.0.11';
// deno-lint-ignore no-import-prefix
import { TextLineStream } from 'jsr:@std/streams@1/text-line-stream';

// Create TypeScript-enabled parser
// deno-lint-ignore no-explicit-any
const ParserWithTS = acorn.Parser.extend(tsPlugin() as any);

// --- Svelte render key (browser-visible render fingerprint) ------------------------------
// Svelte 5 trims render-time whitespace at COMPILE time and bakes the result into the
// server template strings, but it does NOT collapse inter-node whitespace runs — that is
// the browser's job. So two authorings that render identically in a browser can compile to
// server JS that differs only in collapsible whitespace. The render KEY reduces compiled
// server JS to the browser-visible render so those authorings compare equal:
//
//   1. bakedSkeleton — walk the compiled JS string-/template-/comment-aware and collect the
//      static text of every template literal, replacing each `${…}` interpolation with a
//      HOLE. Script logic (non-template-literal code) and script string contents are NOT
//      collected, so a `<script>` reformatting (quotes, semicolons, parens) that leaves the
//      template unchanged produces the SAME skeleton.
//   2. renderKey — strip HTML comments (not visible), collapse ASCII whitespace runs to one
//      space (the browser model), and trim. Two sources with equal keys render identically.
//
// This is the methodology of ../test-svelte-prettier-whitespace/whitespace-safety-check.mjs.
const HOLE = ' ';

function bakedSkeleton(code: string): string {
	const chunks: string[] = [];
	let i = 0;
	const n = code.length;
	const scanString = (quote: string) => {
		i++; // opening quote
		while (i < n) {
			const c = code[i];
			if (c === '\\') {
				i += 2;
				continue;
			}
			if (c === quote) {
				i++;
				return;
			}
			i++;
		}
	};
	// scan a template literal at code[i] === '`', pushing its quasi text (holes for ${…})
	const scanTemplate = () => {
		i++; // opening backtick
		let text = '';
		while (i < n) {
			const c = code[i];
			if (c === '\\') {
				text += code[i + 1] ?? '';
				i += 2;
				continue;
			}
			if (c === '`') {
				i++;
				chunks.push(text);
				return;
			}
			if (c === '$' && code[i + 1] === '{') {
				text += HOLE;
				i += 2;
				scanExpr(); // consume balanced ${ … }
				continue;
			}
			text += c;
			i++;
		}
		chunks.push(text);
	};
	// consume an expression up to the matching unescaped `}` (handles nested () {} [] `` '' "")
	const scanExpr = () => {
		let depth = 1;
		while (i < n && depth > 0) {
			const c = code[i];
			if (c === '`') {
				scanTemplate();
				continue;
			}
			if (c === "'" || c === '"') {
				scanString(c);
				continue;
			}
			if (c === '{') depth++;
			else if (c === '}') depth--;
			if (depth === 0) {
				i++;
				return;
			}
			i++;
		}
	};
	while (i < n) {
		const c = code[i];
		if (c === '`') scanTemplate();
		else if (c === "'" || c === '"') scanString(c);
		else if (c === '/' && code[i + 1] === '/') {
			while (i < n && code[i] !== '\n') i++;
		} else if (c === '/' && code[i + 1] === '*') {
			i += 2;
			while (i < n && !(code[i] === '*' && code[i + 1] === '/')) i++;
			i += 2;
		} else i++;
	}
	return chunks.join(HOLE);
}

// Browser whitespace-collapse model (matches ../test-svelte-prettier-whitespace). A plain
// `\s+ → ' '` collapse is NOT enough: whitespace adjacent to a BLOCK-level element is
// render-insignificant (the browser drops it), so `</div> <div>` and `</div><div>` render
// identically — while the same whitespace between INLINE elements (`</span> <span>`) IS a
// visible space. The model splits the baked HTML at block-tag boundaries and collapses each
// flow segment independently, so block-boundary whitespace vanishes while inline whitespace
// (and text presence) is preserved. `<pre>` runs are kept verbatim (Svelte preserves them).
const BLOCK_TAGS = new Set([
	'address', 'article', 'aside', 'blockquote', 'div', 'dl', 'dt', 'dd', 'fieldset', 'figure',
	'figcaption', 'footer', 'form', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'header', 'hr', 'li',
	'main', 'nav', 'ol', 'p', 'section', 'table', 'thead', 'tbody', 'tr', 'td', 'th', 'ul',
]);
const BR = '\x00'; // block-boundary sentinel (never appears in HTML)

function visibleSegments(body: string): string[] {
	const segments: string[] = [];
	const pre_re = /<pre[\s\S]*?<\/pre>/gi;
	let last = 0;
	let m: RegExpExecArray | null;
	const pushFlow = (html: string) => {
		const marked = html
			// A nested <script>/<style> element's content is not visible render — and
			// reformatting the JS/CSS inside it (quotes, spacing) is a formatter
			// normalization, exactly like the top-level instance script the compile arm
			// ignores by construction. Strip the whole block (its raw text would otherwise
			// read as visible flow text).
			.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '')
			.replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, '')
			.replace(/<!--[\s\S]*?-->/g, '') // comments are not visible render
			.replace(/<\/?([a-zA-Z][\w-]*)\b[^>]*>/g, (_f, tag) =>
				BLOCK_TAGS.has(tag.toLowerCase()) ? BR : '');
		for (const seg of marked.split(BR)) {
			const text = seg.replace(/[ \t\r\n\f]+/g, ' ').trim();
			if (text) segments.push(`text:${text}`);
		}
	};
	while ((m = pre_re.exec(body))) {
		pushFlow(body.slice(last, m.index));
		segments.push(`pre:${m[0]}`); // <pre> kept verbatim
		last = pre_re.lastIndex;
	}
	pushFlow(body.slice(last));
	return segments;
}

// The browser-visible render key of Svelte source: the baked template skeleton reduced to
// its browser-visible flow segments (block-boundary whitespace dropped, inline whitespace and
// text preserved). Two sources with equal keys render identically in a browser.
function svelteRenderKey(source: string): string {
	const compiled = svelteCompile(source, {
		generate: 'server',
		name: 'C',
		filename: 'C.svelte',
	}).js.code;
	return JSON.stringify(visibleSegments(bakedSkeleton(compiled)));
}

interface Request {
	id: number;
	tool: string;
	content: string;
	options?: Record<string, unknown>;
}

// JSON replacer that converts BigInt to string (BigInt can't be serialized natively)
function jsonReplacer(_key: string, value: unknown): unknown {
	return typeof value === 'bigint' ? value.toString() : value;
}

interface Response {
	id: number;
	ok: boolean;
	output?: unknown;
	error?: string;
	duration_ms: number;
}

async function dispatch(
	tool: string,
	content: string,
	options?: Record<string, unknown>,
): Promise<unknown> {
	switch (tool) {
		case '__version_info': {
			return {
				runtime: Deno.version.deno,
				typescript: Deno.version.typescript,
				dependencies: VERSIONS,
			};
		}

		case 'prettier': {
			// Provide default filepath based on parser to help prettier make correct decisions
			// (e.g., typescript parser without filepath hint might add unnecessary JSX disambiguation)
			const filepath = options?.filepath ?? (
				options?.parser === 'typescript'
					? 'file.ts'
					: options?.parser === 'svelte'
					? 'file.svelte'
					: options?.parser === 'css'
					? 'file.css'
					: undefined
			);
			return await prettier.format(content, {
				plugins: [prettierPluginSvelte],
				useTabs: true,
				printWidth: 100,
				singleQuote: true,
				trailingComma: 'none',
				parser: options?.parser as string | undefined,
				filepath: filepath as string | undefined,
			});
		}

		case 'svelte-parse': {
			// Return AST object directly - Rust will serialize with tabs
			return svelteParse(content, { modern: true });
		}

		case 'acorn-typescript-parse': {
			// Return AST object directly - Rust will serialize with tabs.
			// `sourceType` follows the parse goal: 'module' (default) or 'script'
			// for standalone-script fixtures (where `await` is an ordinary
			// identifier and `import`/`export`/`import.meta` are errors).
			const sourceType = (options?.sourceType as 'script' | 'module' | undefined) ?? 'module';
			return ParserWithTS.parse(content, {
				sourceType,
				ecmaVersion: 2025,
				locations: true,
			});
		}

		case 'css-parse': {
			// Return AST object directly - Rust will serialize with tabs
			return parseCss(content);
		}

		case 'svelte-render-key': {
			// The browser-visible render fingerprint (see `svelteRenderKey` above) — the
			// authoritative render-equivalence oracle behind the fixture render-equivalence
			// check. `compile` runs the full semantic ANALYZER, far stricter than
			// `svelte-parse`: it rejects inputs the parser accepts (a TS feature needing a
			// preprocessor, experimental `await`, an illegal default export, a `bind:` to an
			// undeclared or non-assignable target) — errors unrelated to rendering. It throws
			// → the Rust caller falls back to the template-only render_normalize model.
			return svelteRenderKey(content);
		}

		default:
			throw new Error(`Unknown tool: ${tool}`);
	}
}

// Main loop: read JSON-lines from stdin, process, write responses to stdout
const lines = Deno.stdin.readable
	.pipeThrough(new TextDecoderStream())
	.pipeThrough(new TextLineStream());

for await (const line of lines) {
	// Skip empty lines (defensive against stdin noise)
	if (line.trim() === '') continue;

	const start = performance.now();
	let response: Response;

	// Parse request - errors here get a response with id: -1
	let req: Request;
	try {
		req = JSON.parse(line);
	} catch (err) {
		response = {
			id: -1,
			ok: false,
			error: `Invalid JSON request: ${err instanceof Error ? err.message : String(err)}`,
			duration_ms: Math.round(performance.now() - start),
		};
		console.log(JSON.stringify(response, jsonReplacer));
		continue;
	}

	try {
		const output = await dispatch(req.tool, req.content, req.options);
		response = {
			id: req.id,
			ok: true,
			output,
			duration_ms: Math.round(performance.now() - start),
		};
	} catch (err) {
		response = {
			id: req.id,
			ok: false,
			error: err instanceof Error ? err.message : String(err),
			duration_ms: Math.round(performance.now() - start),
		};
	}

	// Use jsonReplacer to handle BigInt values in AST output
	console.log(JSON.stringify(response, jsonReplacer));
}
