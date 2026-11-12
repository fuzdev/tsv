/**
 * The import closure of a set of JS/TS modules, read from their source text: which repo files
 * an entry reaches through its imports, transitively, and what else those files import. Two
 * users, each stating its own rules: the PGO pipeline's stamp key hashes every repo module the
 * pipeline runs (`scripts/pgo/pgo.ts`, `import_closure` there), and
 * `scripts/no_package_closure_test.ts` holds the scripts CI runs with no `node_modules` to
 * importing no package.
 *
 * What counts as an import (`scan_imports`): every static `import … from`, side-effect `import '…'`,
 * `export … from`, and `import(…)` of a string literal (or a template literal with no
 * substitution), wherever it sits in the source: after other code on its line, after a comment,
 * inside a function. An `import(…)` of anything else is reported as computed, with its line,
 * since no text walk can follow it. Comments, strings, template literals, and regular
 * expression literals are skipped, so an import spelled inside one is not one. A clause marked
 * `type` as a whole (`import type …`, `export type … from`) is type-only: it loads nothing.
 * A `/` is read as a regular expression after an operator or a keyword like `return`, else as a
 * division, and a string or regular expression ends at a line break, so a misreading hides at
 * most the rest of its line. A TypeScript type's `import('…')` counts as an import, as Deno's
 * graph counts it; a triple-slash reference or an `@ts-types` comment does not.
 *
 * A specifier beginning `./` or `../` names a file relative to its importer, resolved as a URL
 * path is (percent-escapes decoded, any query or fragment dropped); one beginning `node:` is a
 * builtin; any other is outside the walk: a package, an absolute path or URL, or another scheme.
 * A relative specifier that lands in a `node_modules` directory is a package too, and one that
 * climbs above the root is an error. Files are named by their path relative to the root,
 * `/`-separated on every platform, read through the caller's `read`: the walk itself imports
 * nothing, so the Windows-run scripts that import it stay free of packages.
 *
 * @module
 */

/** One import a module's source spells. */
export interface ImportRecord {
	specifier: string;
	/** The whole clause is `type`, so nothing of it loads. */
	type_only: boolean;
	/** An `import(…)` of a literal, which loads when it runs. */
	dynamic: boolean;
	/** Its 1-based line in the source. */
	line: number;
}

/** What `scan_imports` finds in one module's source. */
export interface ScannedImports {
	imports: ImportRecord[];
	/** The 1-based line of each `import(…)` of something other than a literal. */
	computed: number[];
}

/** A token's kind: a `value` is a number or a regular expression literal, after which a `/`
 * divides. */
type TokenKind = 'name' | 'string' | 'template' | 'value' | 'punct';

interface Token {
	kind: TokenKind;
	/** A name's or punctuator's text, a string's or substitution-free template's contents. */
	value: string;
	/** A template literal with a substitution, which is no literal. */
	substituted?: boolean;
	line: number;
}

/** A keyword after which a `/` starts a regular expression literal, never a division. */
const REGEX_AFTER_KEYWORDS = new Set([
	'return',
	'typeof',
	'instanceof',
	'in',
	'of',
	'new',
	'delete',
	'void',
	'throw',
	'case',
	'do',
	'else',
	'yield',
	'await'
]);

const is_name_start = (c: string): boolean => /[\p{ID_Start}$_\\]/u.test(c);
const is_name_part = (c: string): boolean => /[\p{ID_Continue}$‌‍\\]/u.test(c);
const is_line_break = (c: string): boolean => c === '\n' || c === '\r' || c === ' ' || c === ' ';

/** The punctuators longer than one character the scan tells apart: the member accesses, and
 * the spread, so its dots are no member access. */
const MULTI_PUNCT = ['?.', '...'];

/**
 * The source's tokens, comments dropped, each string and template literal one token holding
 * its contents (a template's substitutions are tokenized as code, after it). A string or
 * regular expression literal ends at a line break even when unterminated, so a misread one
 * hides at most the rest of its line; CR, CRLF, LF, and the Unicode separators each end a line.
 */
const tokenize = (source: string): Token[] => {
	const tokens: Token[] = [];
	const n = source.length;
	let i = source.charCodeAt(0) === 0xfeff ? 1 : 0;
	let line = 1;
	// what each open `{` closes back into: code, or the template literal it substitutes into
	const braces: Array<'code' | 'template'> = [];

	/** Counts the line break at `at`, CRLF once. */
	const count_line = (at: number): void => {
		const c = source[at] ?? '';
		if (c === '\r' && source[at + 1] === '\n') return;
		if (is_line_break(c)) line++;
	};

	const regex_allowed = (): boolean => {
		const last = tokens.at(-1);
		if (last === undefined) return true;
		if (last.kind === 'name') return REGEX_AFTER_KEYWORDS.has(last.value);
		if (last.kind !== 'punct') return false;
		return last.value !== ')' && last.value !== ']' && last.value !== '}';
	};

	/** Reads a template literal from just past its opening backtick or a substitution's `}`, to
	 * its closing backtick or its next `${`. */
	const read_template = (start_line: number, substituted: boolean): void => {
		let text = '';
		while (i < n) {
			const c = source[i]!;
			if (c === '\\') {
				text += source.slice(i, i + 2);
				count_line(i + 1);
				i += 2;
				continue;
			}
			if (c === '`') {
				i++;
				tokens.push({ kind: 'template', value: text, substituted, line: start_line });
				return;
			}
			if (c === '$' && source[i + 1] === '{') {
				i += 2;
				tokens.push({ kind: 'template', value: text, substituted: true, line: start_line });
				braces.push('template');
				return;
			}
			count_line(i);
			text += c;
			i++;
		}
		tokens.push({ kind: 'template', value: text, substituted, line: start_line });
	};

	while (i < n) {
		const c = source[i]!;
		const next = source[i + 1] ?? '';
		if (is_line_break(c)) {
			count_line(i);
			i++;
			continue;
		}
		if (/\s/.test(c)) {
			i++;
			continue;
		}
		// a hashbang, at the very start only
		if (c === '#' && next === '!' && tokens.length === 0 && line === 1) {
			while (i < n && !is_line_break(source[i]!)) i++;
			continue;
		}
		if (c === '/' && next === '/') {
			while (i < n && !is_line_break(source[i]!)) i++;
			continue;
		}
		if (c === '/' && next === '*') {
			const end = source.indexOf('*/', i + 2);
			const stop = end === -1 ? n : end + 2;
			for (let k = i; k < stop; k++) count_line(k);
			i = stop;
			continue;
		}
		if (c === "'" || c === '"') {
			const start_line = line;
			let text = '';
			i++;
			while (i < n && source[i] !== c && !is_line_break(source[i]!)) {
				if (source[i] === '\\') {
					// an escaped line break continues the string
					if (is_line_break(source[i + 1] ?? '')) {
						count_line(i + 1);
						i += source[i + 1] === '\r' && source[i + 2] === '\n' ? 3 : 2;
						continue;
					}
					text += source.slice(i, i + 2);
					i += 2;
					continue;
				}
				text += source[i];
				i++;
			}
			if (source[i] === c) i++;
			tokens.push({ kind: 'string', value: text, line: start_line });
			continue;
		}
		if (c === '`') {
			i++;
			read_template(line, false);
			continue;
		}
		if (c === '/' && regex_allowed()) {
			// a regular expression literal: to its closing `/` outside a class, then its flags
			let in_class = false;
			i++;
			while (i < n && !is_line_break(source[i]!)) {
				const r = source[i]!;
				i++;
				if (r === '\\') {
					if (i < n && !is_line_break(source[i]!)) i++;
				} else if (r === '[') in_class = true;
				else if (r === ']') in_class = false;
				else if (r === '/' && !in_class) break;
			}
			while (i < n && is_name_part(source[i]!)) i++;
			tokens.push({ kind: 'value', value: '/', line });
			continue;
		}
		if (is_name_start(c)) {
			let name = '';
			while (i < n && is_name_part(source[i]!)) {
				name += source[i];
				i++;
			}
			tokens.push({ kind: 'name', value: name, line });
			continue;
		}
		if (/[0-9]/.test(c) || (c === '.' && /[0-9]/.test(next))) {
			i++;
			while (i < n && /[\w.]/.test(source[i]!)) i++;
			tokens.push({ kind: 'value', value: '0', line });
			continue;
		}
		if (c === '{') {
			braces.push('code');
			tokens.push({ kind: 'punct', value: c, line });
			i++;
			continue;
		}
		if (c === '}') {
			i++;
			if (braces.pop() === 'template') {
				read_template(line, true);
				continue;
			}
			tokens.push({ kind: 'punct', value: c, line });
			continue;
		}
		// `?.` but not the `?` of `a?.5:b`
		const multi = MULTI_PUNCT.find((p) => source.startsWith(p, i));
		if (multi !== undefined && !(multi === '?.' && /[0-9]/.test(source[i + 2] ?? ''))) {
			tokens.push({ kind: 'punct', value: multi, line });
			i += multi.length;
			continue;
		}
		tokens.push({ kind: 'punct', value: c, line });
		i++;
	}
	return tokens;
};

const is_punct = (token: Token | undefined, value: string): boolean =>
	token?.kind === 'punct' && token.value === value;

const is_name = (token: Token | undefined, value?: string): boolean =>
	token?.kind === 'name' && (value === undefined || token.value === value);

/** A string, or a template literal with no substitution: what a dynamic import can follow. */
const is_literal = (token: Token | undefined): token is Token =>
	token?.kind === 'string' || (token?.kind === 'template' && token.substituted === false);

/**
 * The specifier of the import or re-export clause starting at `tokens[at]` (just past `import`
 * or `export` and any `type`), or `null` when the tokens there are no such clause: an
 * `export const …`, an `export {…}` of local names, TypeScript's `import x = …`. The grammar:
 * `import` takes an optional default binding, then `,` and a `{…}` or `* as x` binding; `export`
 * takes a `{…}` or `*` (with `as x`); either then needs `from '…'`.
 */
const clause_specifier = (tokens: Token[], at: number, is_import: boolean): Token | null => {
	let j = at;
	if (
		is_import &&
		is_name(tokens[j]) &&
		!(is_name(tokens[j], 'from') && is_literal(tokens[j + 1]))
	) {
		j++;
		if (is_punct(tokens[j], ',')) j++;
		else
			return is_name(tokens[j], 'from') && tokens[j + 1]?.kind === 'string' ? tokens[j + 1]! : null;
	}
	if (is_punct(tokens[j], '*')) {
		j++;
		if (is_name(tokens[j], 'as')) {
			j++;
			if (!is_name(tokens[j]) && tokens[j]?.kind !== 'string') return null;
			j++;
		}
	} else if (is_punct(tokens[j], '{')) {
		j++;
		while (!is_punct(tokens[j], '}')) {
			const t = tokens[j];
			if (t === undefined) return null;
			if (t.kind !== 'name' && t.kind !== 'string' && !is_punct(t, ',')) return null;
			j++;
		}
		j++;
	} else return null;
	return is_name(tokens[j], 'from') && tokens[j + 1]?.kind === 'string' ? tokens[j + 1]! : null;
};

/** Whether the token after a clause's leading `type` makes it a modifier rather than a default
 * import's name: a name (`type X`, `type from from '…'`), `{`, or `*` follows it, but not
 * `from '…'`, `,`, or `=`. */
const is_type_modifier = (tokens: Token[], at: number): boolean => {
	const t = tokens[at];
	if (is_punct(t, '{') || is_punct(t, '*')) return true;
	if (!is_name(t)) return false;
	return !(t!.value === 'from' && tokens[at + 1]?.kind === 'string');
};

/** Every import `source` spells, and the line of each `import(…)` no walk can follow. */
export const scan_imports = (source: string): ScannedImports => {
	const tokens = tokenize(source);
	const imports: ImportRecord[] = [];
	const computed: number[] = [];
	for (let k = 0; k < tokens.length; k++) {
		const t = tokens[k]!;
		if (t.kind !== 'name' || (t.value !== 'import' && t.value !== 'export')) continue;
		const prev = tokens[k - 1];
		// a property (`x.import`, `x?.import`) or an object key (`{import: 1}`) is no import
		if (is_punct(prev, '.') || is_punct(prev, '?.')) continue;
		const next = tokens[k + 1];
		if (next === undefined || is_punct(next, ':')) continue;
		const is_import = t.value === 'import';
		if (is_import && is_punct(next, '.')) continue; // `import.meta`
		if (is_import && is_punct(next, '(')) {
			const arg = tokens[k + 2];
			const after = tokens[k + 3];
			if (is_literal(arg) && (is_punct(after, ')') || is_punct(after, ','))) {
				imports.push({ specifier: arg.value, type_only: false, dynamic: true, line: t.line });
			} else computed.push(t.line);
			continue;
		}
		if (is_import && next.kind === 'string') {
			imports.push({ specifier: next.value, type_only: false, dynamic: false, line: t.line });
			continue;
		}
		const type_only = is_name(next, 'type') && is_type_modifier(tokens, k + 2);
		const specifier = clause_specifier(tokens, type_only ? k + 2 : k + 1, is_import);
		if (specifier === null) continue;
		imports.push({ specifier: specifier.value, type_only, dynamic: false, line: t.line });
	}
	return { imports, computed };
};

/** An import's specifier against its importer, in the walk's terms. */
export type ResolvedImport =
	| { kind: 'file'; path: string }
	| { kind: 'builtin' }
	| { kind: 'package' }
	| { kind: 'above_root' };

/**
 * Resolves `specifier`, imported by the module at root-relative `importer`. A relative
 * specifier becomes a root-relative `/`-separated path, `.` and `..` segments folded, its
 * percent-escapes decoded and any `?query` or `#fragment` dropped, as a URL resolves.
 */
export const resolve_import = (importer: string, specifier: string): ResolvedImport => {
	if (specifier.startsWith('node:')) return { kind: 'builtin' };
	if (!specifier.startsWith('./') && !specifier.startsWith('../')) return { kind: 'package' };
	const bare = specifier.replace(/[?#].*$/s, '');
	const segments = importer.split('/').slice(0, -1);
	for (const segment of bare.split('/')) {
		if (segment === '' || segment === '.') continue;
		if (segment === '..') {
			if (segments.length === 0) return { kind: 'above_root' };
			segments.pop();
		} else segments.push(decodeURIComponent(segment));
	}
	if (segments.includes('node_modules')) return { kind: 'package' };
	return { kind: 'file', path: segments.join('/') };
};

/** An import, with the root-relative path of the module that spells it. */
export interface FoundImport extends ImportRecord {
	from: string;
}

/** How a walk treats what the two users weigh differently. */
export interface ImportClosureOptions {
	/** Reads the module at a root-relative, `/`-separated path; throws when it can't. */
	read: (path: string) => string;
	/** Walk into a type-only import's file (the stamp key: its bytes are an input), or stop
	 * there (the load checks: it loads nothing). */
	follow_type_only: boolean;
}

/** What a walk reached. */
export interface ImportClosure {
	/** Every file reached, entries first, each once. */
	files: string[];
	/** Each `node:*` import. */
	builtins: FoundImport[];
	/** Each import outside the walk: a package, a path into `node_modules`, an absolute
	 * specifier or URL. */
	packages: FoundImport[];
	/** Each `import(…)` of something other than a literal. */
	computed: Array<{ from: string; line: number }>;
}

/** Whether the walk reads a file's imports: JS and TS sources, never JSON or other data. */
const is_module_source = (path: string): boolean => /\.[cm]?[jt]sx?$/.test(path);

/**
 * The closure of `entries` (root-relative, `/`-separated).
 * @throws `Error` - when a relative import climbs above the root, or `read` throws
 */
export const walk_import_closure = (
	entries: readonly string[],
	options: ImportClosureOptions
): ImportClosure => {
	const seen = new Set<string>();
	const closure: ImportClosure = { files: [], builtins: [], packages: [], computed: [] };
	const visit = (path: string): void => {
		if (seen.has(path)) return;
		seen.add(path);
		closure.files.push(path);
		if (!is_module_source(path)) return;
		const scanned = scan_imports(options.read(path));
		for (const line of scanned.computed) closure.computed.push({ from: path, line });
		for (const record of scanned.imports) {
			const found: FoundImport = { ...record, from: path };
			const resolved = resolve_import(path, record.specifier);
			if (resolved.kind === 'builtin') closure.builtins.push(found);
			else if (resolved.kind === 'package') closure.packages.push(found);
			else if (resolved.kind === 'above_root') {
				throw new Error(`${path}:${record.line}: ${record.specifier} climbs above the root`);
			} else if (options.follow_type_only || !record.type_only) visit(resolved.path);
		}
	};
	for (const entry of entries) visit(entry);
	return closure;
};
