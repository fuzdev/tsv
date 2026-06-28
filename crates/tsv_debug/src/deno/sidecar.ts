// Deno Sidecar for tsv_debug
// Long-running process for JS tools. Communicates via JSON-lines over stdio.

// ⚠ SYNC — each canonical version below is pinned in THREE places that must stay
// identical: (1) this VERSIONS object, (2) the static `import` specifiers just below
// (a static import can't interpolate a const, so the literal is repeated by necessity),
// and (3) benches/js/package.json (read by benches/js/lib/versions.ts).
// These can't be DRYed: the release binary embeds this file as a string and runs it
// WITHOUT that package.json, so it can't read the bench's pins at runtime.
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
	prettier: '3.9.0',
	'prettier-plugin-svelte': '4.1.1',
	svelte: '5.56.1',
	acorn: '8.16.0',
	'@sveltejs/acorn-typescript': '1.0.10',
} as const;

// TODO verify there's not a better solution to use deno.json here, see the above NOTE too
// Imports are like this because these don't have the deno.json when used by the release binary.
// deno-lint-ignore no-import-prefix
import * as prettier from 'npm:prettier@3.9.0';
// deno-lint-ignore no-import-prefix
import prettierPluginSvelte from 'npm:prettier-plugin-svelte@4.1.1';
// deno-lint-ignore no-import-prefix
import { parse as svelteParse, parseCss } from 'npm:svelte@5.56.1/compiler';
// deno-lint-ignore no-import-prefix
import * as acorn from 'npm:acorn@8.16.0';
// deno-lint-ignore no-import-prefix
import { tsPlugin } from 'npm:@sveltejs/acorn-typescript@1.0.10';
// deno-lint-ignore no-import-prefix
import { TextLineStream } from 'jsr:@std/streams@1/text-line-stream';

// Create TypeScript-enabled parser
// deno-lint-ignore no-explicit-any
const ParserWithTS = acorn.Parser.extend(tsPlugin() as any);

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
