/**
 * Hand-written types for `@fuzdev/tsv` — mirrors `@fuzdev/tsv_wasm`'s
 * surface (same option interfaces, same overloads, same `tsv_ast` re-export),
 * so the two packages type-check interchangeably except for what a WASM engine
 * needs and this one doesn't: `init()` and `init_sync()` (nothing here needs
 * initializing), `wasm_module` (no compiled module to hand a worker),
 * `reinstantiate()` (no instance to poison), and `IgnoreStack`'s `free()` /
 * `[Symbol.dispose]` (a GC-managed native object has no handle to release).
 * That list is the whole delta — `scripts/test_napi_npm.ts` diffs the two
 * packages' export names rather than trusting it. The
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
 * `undefined`-only rather than omitted: a set `sourceType` throws, but spelling
 * the inapplicable source type `undefined` forwards one bag to whichever parser,
 * exactly as the runtime does.
 */
export interface ParseOptions {
	/**
	 * Emit per-node `loc` (line/column) — the drop-in acorn/svelte wire.
	 * `false` emits the span-only wire (much smaller; Svelte also omits
	 * `name_loc`): `loc` stays derivable from `start`/`end` plus the source,
	 * via this package's own `reconstruct_locations` / `create_locator` /
	 * `loc_of` (which throw on the two Svelte shapes the span-only wire can't
	 * disambiguate — see `locations.d.ts`). Inert for CSS (its wire has no `loc`).
	 * @default true
	 */
	locations?: boolean | undefined;
	/**
	 * Not accepted here — Svelte's `<script>` is always a module and CSS has no
	 * goal, so a set `sourceType` throws. See `TypeScriptParseOptions`.
	 */
	sourceType?: undefined;
}

/** The TypeScript parsers' bag: the same keys, with `sourceType` settable. */
export interface TypeScriptParseOptions {
	/** As `ParseOptions.locations`. @default true */
	locations?: boolean | undefined;
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors. A script is also
	 * **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a
	 * module is always strict.
	 * @default 'module'
	 */
	sourceType?: 'script' | 'module' | undefined;
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
	 * goal, so a set `sourceType` throws. Declared (as `undefined`) rather than
	 * omitted so one bag still forwards to whichever formatter: spell the
	 * inapplicable source type `undefined` and this type accepts it, exactly as
	 * the runtime does.
	 */
	sourceType?: undefined;
}

/** The TypeScript formatter's bag: the same key, settable. */
export interface TypeScriptFormatOptions {
	/**
	 * Parse goal: at `'script'`, `await` is an ordinary identifier and
	 * `import`/`export`/`import.meta` are syntax errors. A script is also
	 * **sloppy** unless its own `"use strict"` directive prologue makes it
	 * strict, so `with` and the legacy octal literals/escapes parse there; a
	 * module is always strict.
	 *
	 * Omitted, the source is formatted as a **module, retried as a script** if
	 * that parse fails — so a legacy sloppy script formats without naming a
	 * grammar, while anything the module grammar accepts is never reinterpreted
	 * (the printer does not read the goal, so no output changes). A set value is
	 * exact: `'module'` refuses a script-only source rather than retrying.
	 * `parse_typescript` has no such fallback — its wire's `Program.sourceType`
	 * is a claim, and omitting the key there means `'module'`.
	 */
	sourceType?: 'script' | 'module' | undefined;
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
 * two type-check interchangeably — save the WASM class's wasm-bindgen
 * lifecycle pair, `free()` and `[Symbol.dispose]()`: this class is
 * GC-managed and has neither, so a `using stack = new IgnoreStack()` or an
 * explicit `stack.free()` is a WASM-only spelling.
 */
export class IgnoreStack {
	constructor();
	/** Push one directory's `.gitignore`. */
	push_gitignore(anchor: string, content: string): void;
	/** Pop the most recently pushed `.gitignore` layer. */
	pop_gitignore(): void;
	/** Push one directory's `.formatignore`, applied after every `.gitignore`. */
	push_formatignore(anchor: string, content: string): void;
	/** Push one directory's `.prettierignore` (read where a directory has no `.formatignore`), applied after every `.gitignore`. */
	push_prettierignore(anchor: string, content: string): void;
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
	/** The shadow warning for the first ancestor directory of `rel` discovery prunes under a tsv-layer re-include, else `undefined`. `loose_root` is the format root outside a git repo. */
	path_shadow_warning(rel: string, loose_root?: string): string | undefined;
	/** The argument error for a named file tsv doesn't format, else `undefined`. */
	unsupported_extension_error(path: string): string | undefined;
	/** The warning for a directory the build-output heuristic or an ignore rule pruned under a tsv-layer re-include, else `undefined`. `loose_root` is the format root outside a git repo. */
	shadow_warning(dir: string, loose_root?: string): string | undefined;
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
	/** The warning for an in-tree `.gitignore` that is a symbolic link (git does not follow one). */
	gitignore_symlink_warning(path: string): string;
	/** The traversal error for a relative root the working directory cannot resolve. */
	unresolvable_root_error(root: string): string;
	/** The warning for a named path an ignore file excludes, else `undefined` (also for a named file a `.formatignore` / `.prettierignore` excludes, skipped quietly). `loose_root` is the format root outside a git repo. */
	excluded_argument_warning(
		display: string,
		rel: string,
		is_dir: boolean,
		loose_root?: string
	): string | undefined;
}
