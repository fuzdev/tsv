/**
 * Hand-written types for `@fuzdev/tsv` — `@fuzdev/tsv-wasm`'s root surface (the same facade
 * declarations, re-exported from the staged `facade_format.d.ts` / `facade_strip.d.ts` /
 * `facade_imports.d.ts` / `facade_error.d.ts`), so the two packages type-check interchangeably
 * except for what a WASM engine needs and this one doesn't: `init()` and `init_sync()` (nothing
 * here needs initializing), `wasm_module` (no compiled module to hand a worker),
 * `reinstantiate()` (no instance to poison), and `IgnoreStack`'s `free()` / `[Symbol.dispose]`
 * (a GC-managed native object has no handle to release). That list is the whole delta —
 * `scripts/test_napi_npm.ts` diffs the two packages' export names rather than trusting it. The
 * parse family, the AST types and the `locations.js` helper are the `./parse` entry's
 * (`parse.d.ts`), not this root's.
 *
 * Relative specifiers carry the **`.js`** extension, here and in every
 * `import(...)` below: under `moduleResolution: node16`/`nodenext` a relative
 * specifier inside a declaration file must have the runtime extension or the
 * consumer gets TS2834, and TypeScript resolves `./facade_format.js` to
 * `./facade_format.d.ts`. Same rule the wasm packages' generated declarations follow.
 */

// The format surface is the shared facade's, declared once beside it (`facade_format.d.ts`,
// staged in from `crates/tsv_wasm/npm/`, and named apart from the `api.js` module beside it,
// which does not export these). Re-exported by NAME.
export type {
	FormatOptions,
	TypeScriptFormatOptions,
	FormatSuccess,
	FormatFailure,
	FormatResult,
	FormatError,
	FormatRefusedError
} from './facade_format.js';
export type {
	StripOptions,
	StripSuccess,
	StripFailure,
	StripResult,
	StripError,
	StripUnsupportedKind,
	StripUnsupportedError
} from './facade_strip.js';
export type {
	TypeScriptImportsOptions,
	ImportsOptions,
	ImportRecordKind,
	ImportRecordSpan,
	ImportRecordSpecifier,
	ImportRecord,
	ImportsSuccess,
	ImportsFailure,
	ImportsResult
} from './facade_imports.js';
export type { TsvError, TsvParseError } from './facade_error.js';
export { format_svelte, format_typescript, format_css } from './facade_format.js';
export { strip_typescript } from './facade_strip.js';
export { parse_imports_typescript, parse_imports_svelte } from './facade_imports.js';

/**
 * The gitignore-aware matcher stack — the same layering and prune decisions
 * `tsv format` makes natively, so a JS traversal agrees with the CLI by
 * construction. Push layers shallowest-first; anchors are `/`-separated and
 * relative to the format root (`''` = the root).
 *
 * Mirrors `@fuzdev/tsv-wasm`'s class method for method, including the
 * `string | undefined` (never `null`) of the maybe-a-warning methods, so the
 * two type-check interchangeably — save the WASM class's wasm-bindgen
 * lifecycle pair, `free()` and `[Symbol.dispose]()`: this class is
 * GC-managed and has neither, so a `using stack = new IgnoreStack()` or an
 * explicit `stack.free()` is a WASM-only spelling. The two agree on every call
 * these declarations admit; argument checking past them is each engine's own
 * (the class is the binding's, with no facade reading its arguments), so a
 * wrong-typed argument is refused or coerced differently by each.
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
	/** Pop the most recently pushed override layer. */
	pop_override(): void;
	/** Whether `path` is ignored; `is_dir` makes trailing-`/` patterns apply (a symbolic link is not a directory here, as for git). */
	is_ignored(path: string, is_dir: boolean): boolean;
	/** Discovery verdict for a child directory: `'descend'`, `'prune'`, or `'prune_warn'`. */
	classify_dir(child_rel: string, heuristic_active: boolean): 'descend' | 'prune' | 'prune_warn';
	/** Whether a child file has a formattable extension and isn't ignored. */
	should_format_file(child_rel: string): boolean;
	/** Whether an ancestor directory of `rel` would be pruned by discovery. */
	is_path_pruned(rel: string): boolean;
	/** The shadow warning for the first ancestor directory of `rel` discovery prunes under an override-layer re-include, else `undefined`. `loose_root` is the format root outside a git repo. */
	path_shadow_warning(rel: string, loose_root?: string): string | undefined;
	/** The argument error for a named file tsv doesn't format, else `undefined`. */
	unsupported_extension_error(path: string): string | undefined;
	/** The warning for a directory the build-output heuristic or an ignore rule pruned under an override-layer re-include, else `undefined`. `loose_root` is the format root outside a git repo. */
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
	/** The warning for a named path an ignore file excludes, else `undefined` (also for a named file a `.formatignore` / `.prettierignore` excludes, skipped quietly). `is_dir` is the kind as the matcher reads it — never for a symbolic link, whatever it points at. `loose_root` is the format root outside a git repo. */
	excluded_argument_warning(
		display: string,
		rel: string,
		is_dir: boolean,
		loose_root?: string
	): string | undefined;
}
