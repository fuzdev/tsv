/**
 * Hand-written types for `@fuzdev/tsv` — mirrors `@fuzdev/tsv_wasm`'s
 * surface (same option interfaces, same overloads, same `tsv_ast` re-export),
 * so the two packages type-check interchangeably except for what a WASM engine
 * needs and this one doesn't: `init()` and `init_sync()` (nothing here needs
 * initializing) and `wasm_module` (no compiled module to hand a worker). The
 * `locations.js` helper's types are appended at stage time by
 * `scripts/build_napi_packages.ts`, alongside the copy of the helper itself.
 *
 * Relative specifiers carry the **`.js`** extension, here and in every
 * `import(...)` below: under `moduleResolution: node16`/`nodenext` a relative
 * specifier inside a declaration file must have the runtime extension or the
 * consumer gets TS2834, and TypeScript resolves `./tsv_ast.js` to
 * `./tsv_ast.d.ts`. Same rule the wasm packages' generated declarations follow.
 */
export type * from './tsv_ast.js';

/**
 * Options accepted by `parse_svelte` / `parse_css` (and their `_json`
 * siblings). The parse goal is TypeScript's alone, so it is declared here as
 * `undefined`-only rather than omitted: a set `goal` throws, but spelling the
 * inapplicable goal `undefined` forwards one bag to whichever parser, exactly
 * as the runtime does.
 */
export interface ParseOptions {
	/**
	 * Emit per-node `loc` (line/column) — the drop-in acorn/svelte wire.
	 * `false` emits the span-only wire (~46% smaller; Svelte also omits
	 * `name_loc`): `loc` stays derivable from `start`/`end` plus the source,
	 * via this package's own `reconstruct_locations` / `create_locator` /
	 * `loc_of`. Inert for CSS (its wire has no `loc`).
	 * @default true
	 */
	locations?: boolean | undefined;
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `goal` throws. See `TypeScriptParseOptions`.
	 */
	goal?: undefined;
}

/** The TypeScript parsers' bag: the same keys, with `goal` settable. */
export interface TypeScriptParseOptions {
	/** As `ParseOptions.locations`. @default true */
	locations?: boolean | undefined;
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module' | undefined;
}

/**
 * Options accepted by `format_svelte` / `format_css`. Formatting itself is
 * non-configurable and the parse goal is TypeScript's alone, so these carry no
 * settable key. Every unknown key throws, `locations` included: that option
 * shapes the parse wire, and format emits no wire.
 */
export interface FormatOptions {
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `goal` throws. Declared (as `undefined`) rather than
	 * omitted so one bag still forwards to whichever formatter: spell the
	 * inapplicable goal `undefined` and this type accepts it, exactly as the
	 * runtime does.
	 */
	goal?: undefined;
}

/** The TypeScript formatter's bag: the same key, settable. */
export interface TypeScriptFormatOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors.
	 * @default 'module'
	 */
	goal?: 'script' | 'module' | undefined;
}

export function parse_svelte(source: string, options: ParseOptions & { locations: false }): any;
export function parse_svelte(source: string, options?: ParseOptions): import('./tsv_ast.js').Root;
export function parse_svelte_json(source: string, options?: ParseOptions): string;

export function parse_typescript(
	source: string,
	options: TypeScriptParseOptions & { locations: false }
): any;
export function parse_typescript(
	source: string,
	options?: TypeScriptParseOptions
): import('./tsv_ast.js').Program;
export function parse_typescript_json(source: string, options?: TypeScriptParseOptions): string;

export function parse_css(
	source: string,
	options?: ParseOptions
): import('./tsv_ast.js').StyleSheetFile;
export function parse_css_json(source: string, options?: ParseOptions): string;

export function format_svelte(source: string, options?: FormatOptions): string;
export function format_typescript(source: string, options?: TypeScriptFormatOptions): string;
export function format_css(source: string, options?: FormatOptions): string;

/**
 * The gitignore-aware matcher stack — the same layering and prune decisions
 * `tsv format` makes natively, so a JS traversal agrees with the CLI by
 * construction. Push layers shallowest-first; anchors are `/`-separated and
 * relative to the format root (`''` = the root).
 *
 * Mirrors `@fuzdev/tsv_wasm`'s class method for method, including the
 * `string | undefined` (never `null`) of the maybe-a-warning methods, so the
 * two type-check interchangeably.
 */
export class IgnoreStack {
	constructor();
	/** Push one directory's `.gitignore`. */
	push_gitignore(anchor: string, content: string): void;
	/** Pop the most recently pushed `.gitignore` layer. */
	pop_gitignore(): void;
	/** Push one directory's `.formatignore` / `.prettierignore`, applied after every `.gitignore`. */
	push_tsv(anchor: string, content: string): void;
	/** Pop the most recently pushed tsv layer. */
	pop_tsv(): void;
	/** Whether `path` is ignored; `is_dir` makes trailing-`/` patterns apply. */
	is_ignored(path: string, is_dir: boolean): boolean;
	/** Discovery verdict for a child directory: `'descend'`, `'prune'`, or `'prune_warn'`. */
	classify_dir(name: string, child_rel: string, heuristic_active: boolean): string;
	/** Whether a child file has a formattable extension and isn't ignored. */
	should_format_file(name: string, child_rel: string): boolean;
	/** Whether an ancestor directory of `rel` would be pruned by discovery. */
	is_path_pruned(rel: string): boolean;
	/** The argument error for a named file tsv doesn't format, else `undefined`. */
	unsupported_extension_error(path: string): string | undefined;
	/** The warning text for a directory pruned by the build-output heuristic. */
	heuristic_shadow_warning(dir: string): string;
	/** The `.prettierignore`-outside-a-repo warning, else `undefined`. */
	prettierignore_outside_repo_warning(
		dir: string,
		in_repo: boolean,
		has_prettierignore: boolean,
		has_formatignore: boolean
	): string | undefined;
	/** The `.prettierignore`-shadowed-by-`.formatignore` warning, else `undefined`. */
	prettierignore_shadowed_warning(
		dir: string,
		in_repo: boolean,
		has_prettierignore: boolean,
		has_formatignore: boolean
	): string | undefined;
	/** Whether no layer carries any rule. */
	is_empty(): boolean;
}
