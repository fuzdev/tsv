/**
 * Corpus loading for benchmarks.
 *
 * Provides a pluggable interface for loading source files from different sources.
 * The default implementation loads from ~/dev/ repos.
 */

import { walk } from '@std/fs/walk';
import { basename, dirname, extname, join } from '@std/path';

import type { CorpusStats, Language, Logger, SourceFile } from './types.ts';

/**
 * Interface for loading benchmark corpus from different sources.
 *
 * Implementations can load from:
 * - Local directories (default)
 * - Remote URLs
 * - Git repositories
 * - Fixture directories
 * - etc.
 */
export interface CorpusLoader {
	/** Human-readable name for this corpus source */
	readonly name: string;

	/**
	 * Load all source files from this corpus.
	 * @param logger Optional logger for progress output
	 * @returns Loaded files and statistics
	 */
	load(logger?: Logger): Promise<{ files: SourceFile[]; stats: CorpusStats }>;
}

//
// Shared Utilities
//

/** Detect language from file extension */
export function detectLanguage(path: string): Language | null {
	const ext = extname(path).toLowerCase();
	switch (ext) {
		case '.svelte':
		case '.html':
			return 'svelte';
		case '.ts':
		case '.js':
			return 'typescript';
		case '.css':
			return 'css';
		default:
			return null;
	}
}

/** Default exclusion patterns */
const DEFAULT_EXCLUSIONS = [
	'.d.ts', // Declaration files
	'/node_modules/',
	'/.svelte-kit/',
	'/.gro/',
	'/build/',
	'/dist/',
];

/**
 * Check if a file has a companion options.json (non-default prettier settings).
 * Checks two patterns:
 * - Same directory: `dir/options.json` (prettier-plugin-svelte formatting samples)
 * - Sibling file: `name.options.json` (prettier-plugin-svelte printer samples)
 */
async function hasCompanionOptions(filePath: string): Promise<boolean> {
	const dir = dirname(filePath);
	const base = basename(filePath);
	const nameWithoutExt = base.replace(/\.[^.]+$/, '');

	try {
		// Check dir/options.json (formatting samples pattern)
		await Deno.stat(join(dir, 'options.json'));
		return true;
	} catch {
		// Not found, check sibling pattern
	}

	try {
		// Check name.options.json (printer samples pattern)
		await Deno.stat(join(dir, `${nameWithoutExt}.options.json`));
		return true;
	} catch {
		return false;
	}
}

/** Check if file should be excluded based on default patterns */
export function shouldExclude(path: string, exclusions = DEFAULT_EXCLUSIONS): boolean {
	const name = basename(path);
	for (const pattern of exclusions) {
		if (pattern.startsWith('/')) {
			// Directory pattern
			if (path.includes(pattern)) return true;
		} else {
			// Filename pattern
			if (name.includes(pattern)) return true;
		}
	}
	return false;
}

/** Compute corpus statistics from files */
export function computeStats(files: SourceFile[], source: string): CorpusStats {
	const stats: CorpusStats = {
		totalFiles: files.length,
		totalBytes: files.reduce((sum, f) => sum + f.bytes, 0),
		byLanguage: {
			svelte: { files: 0, bytes: 0 },
			typescript: { files: 0, bytes: 0 },
			css: { files: 0, bytes: 0 },
		},
		repos: [source],
	};

	for (const file of files) {
		stats.byLanguage[file.language].files++;
		stats.byLanguage[file.language].bytes += file.bytes;
	}

	return stats;
}

/** Log corpus statistics */
export function logStats(stats: CorpusStats, logger: Logger): void {
	logger(`\nCorpus loaded:`);
	logger(`  Total: ${stats.totalFiles} files, ${(stats.totalBytes / 1024 / 1024).toFixed(2)} MB`);
	logger(`  Svelte: ${stats.byLanguage.svelte.files} files`);
	logger(`  TypeScript: ${stats.byLanguage.typescript.files} files`);
	logger(`  CSS: ${stats.byLanguage.css.files} files`);
}

/** Group files by language for targeted benchmarks */
export function groupByLanguage(files: SourceFile[]): Record<Language, SourceFile[]> {
	return {
		svelte: files.filter((f) => f.language === 'svelte'),
		typescript: files.filter((f) => f.language === 'typescript'),
		css: files.filter((f) => f.language === 'css'),
	};
}

//
// Default Implementation: ~/dev/ repos
//

/** Per-repo configuration overrides */
export interface RepoConfig {
	name: string;
	/** Override srcDir for this repo (default: global srcDir) */
	srcDir?: string;
	/** Override extensions for this repo (default: global extensions) */
	extensions?: string[];
	/** Override exclusions for this repo (default: global exclusions) */
	exclusions?: string[];
}

/** A repo entry is either a name string or a config object */
export type RepoEntry = string | RepoConfig;

/** Configuration for DevReposLoader */
export interface DevReposLoaderOptions {
	/** Base directory containing repos (default: ~/dev) */
	baseDir?: string;
	/** List of repo names to load (default: hardcoded list) */
	repos?: RepoEntry[];
	/** Subdirectory within each repo to scan (default: src) */
	srcDir?: string;
	/** File extensions to include (default: svelte, ts, js, css) */
	extensions?: string[];
	/** Patterns to exclude (default: DEFAULT_EXCLUSIONS) */
	exclusions?: string[];
}

/**
 * Default repos for the dev loader.
 * SvelteKit projects from ~/dev/web.code-workspace (those with src/routes/).
 * Object entries override per-repo srcDir/extensions/exclusions.
 */
const DEFAULT_REPOS: RepoEntry[] = [
	// Large apps
	'zzz',
	'mageguild',
	'tx',
	// Fuz ecosystem
	'fuz.dev',
	'fuz_app',
	'fuz_blog',
	'fuz_css',
	'fuz_docs',
	'fuz_code',
	'fuz_gitops',
	'fuz_mastodon',
	'fuz_template',
	'fuz_ui',
	'fuz_util',
	// Build tooling
	'gro',
	'svelte-docinfo',
	'tsv.dev',
	// Applications
	'visionesdelcaribe.org',
	// Personal sites
	'webdevladder.net',
	'ryanatkn.com',
	// External projects (monorepo subpaths — baseDir/name/srcDir still resolves)
	'svelte/packages/svelte',
	'svelte.dev/apps/svelte.dev',
	'svelte.dev/packages/repl',
	'svelte.dev/packages/site-kit',
	// prettier-plugin-svelte test cases (.html files treated as Svelte)
	// output.html = prettier-formatted (canonical), input.html = unformatted source
	// Both included — input.html surfaces normalization differences
	{
		name: 'prettier-plugin-svelte',
		srcDir: 'test',
		extensions: ['html'],
	},
	// TODO: svelte/packages/svelte/tests (7124 files — needs per-repo srcDir override)
];

/**
 * Loads corpus from ~/dev/ repositories.
 * This is the default corpus loader used for benchmarking against real-world code.
 */
export class DevReposLoader implements CorpusLoader {
	readonly name = 'dev-repos';
	readonly #options: Required<DevReposLoaderOptions>;

	constructor(options: DevReposLoaderOptions = {}) {
		const homeDir = Deno.env.get('HOME');
		if (!homeDir && !options.baseDir) {
			throw new Error('HOME environment variable not set and no baseDir provided');
		}

		this.#options = {
			baseDir: options.baseDir ?? `${homeDir}/dev`,
			repos: options.repos ?? DEFAULT_REPOS,
			srcDir: options.srcDir ?? 'src',
			extensions: options.extensions ?? ['svelte', 'ts', 'js', 'css'],
			exclusions: options.exclusions ?? DEFAULT_EXCLUSIONS,
		};
	}

	async load(logger: Logger = console.log): Promise<{ files: SourceFile[]; stats: CorpusStats }> {
		const { baseDir, repos, srcDir, extensions, exclusions } = this.#options;

		logger(`Loading ${repos.length} repos from ${baseDir}`);

		const allFiles: SourceFile[] = [];
		const loadedRepos: string[] = [];

		for (const entry of repos) {
			const config = typeof entry === 'string' ? { name: entry } : entry;
			const repoSrcDir = config.srcDir ?? srcDir;
			const repoExtensions = config.extensions ?? extensions;
			const repoExclusions = config.exclusions ?? exclusions;
			const repoPath = `${baseDir}/${config.name}`;
			const srcPath = `${repoPath}/${repoSrcDir}`;

			try {
				await Deno.stat(srcPath);
			} catch {
				logger(`  ${config.name}: not found, skipping`);
				continue;
			}

			const repoFiles = await this.#loadDirectory(srcPath, repoExtensions, repoExclusions);
			if (repoFiles.length > 0) {
				allFiles.push(...repoFiles);
				loadedRepos.push(config.name);
				logger(`  ${config.name}: ${repoFiles.length} files`);
			}
		}

		const stats = computeStats(allFiles, this.name);
		stats.repos = loadedRepos;
		logStats(stats, logger);

		return { files: allFiles, stats };
	}

	async #loadDirectory(
		dirPath: string,
		extensions: string[],
		exclusions: string[],
	): Promise<SourceFile[]> {
		const files: SourceFile[] = [];

		for await (const entry of walk(dirPath, { exts: extensions, includeDirs: false })) {
			if (shouldExclude(entry.path, exclusions)) continue;

			const language = detectLanguage(entry.path);
			if (!language) continue;

			// Skip .html files with companion options.json (non-default prettier settings)
			if (entry.path.endsWith('.html') && (await hasCompanionOptions(entry.path))) {
				continue;
			}

			try {
				const content = await Deno.readTextFile(entry.path);
				files.push({
					path: entry.path,
					content,
					language,
					bytes: new TextEncoder().encode(content).length,
				});
			} catch (e) {
				console.warn(`Warning: Could not read ${entry.path}: ${e}`);
			}
		}

		return files;
	}
}

//
// Alternative Implementation: Single Directory
//

/** Configuration for DirectoryLoader */
export interface DirectoryLoaderOptions {
	/** Directory to scan */
	path: string;
	/** File extensions to include (default: svelte, ts, js, css) */
	extensions?: string[];
	/** Patterns to exclude (default: DEFAULT_EXCLUSIONS) */
	exclusions?: string[];
	/** Whether to recurse into subdirectories (default: true) */
	recursive?: boolean;
}

/**
 * Loads corpus from a single directory.
 * Useful for benchmarking against a specific project or fixture set.
 */
export class DirectoryLoader implements CorpusLoader {
	readonly name: string;
	readonly #options: Required<DirectoryLoaderOptions>;

	constructor(options: DirectoryLoaderOptions) {
		this.name = `directory:${options.path}`;
		this.#options = {
			path: options.path,
			extensions: options.extensions ?? ['svelte', 'ts', 'js', 'css'],
			exclusions: options.exclusions ?? DEFAULT_EXCLUSIONS,
			recursive: options.recursive ?? true,
		};
	}

	async load(logger: Logger = console.log): Promise<{ files: SourceFile[]; stats: CorpusStats }> {
		const { path, extensions, exclusions, recursive } = this.#options;

		logger(`Loading from ${path}`);

		const files: SourceFile[] = [];

		try {
			await Deno.stat(path);
		} catch {
			throw new Error(`Directory not found: ${path}`);
		}

		if (recursive) {
			for await (const entry of walk(path, { exts: extensions, includeDirs: false })) {
				if (shouldExclude(entry.path, exclusions)) continue;

				const language = detectLanguage(entry.path);
				if (!language) continue;

				try {
					const content = await Deno.readTextFile(entry.path);
					files.push({
						path: entry.path,
						content,
						language,
						bytes: new TextEncoder().encode(content).length,
					});
				} catch (e) {
					console.warn(`Warning: Could not read ${entry.path}: ${e}`);
				}
			}
		} else {
			for await (const entry of Deno.readDir(path)) {
				if (!entry.isFile) continue;

				const ext = extname(entry.name).slice(1);
				if (!extensions.includes(ext)) continue;

				const filePath = `${path}/${entry.name}`;
				if (shouldExclude(filePath, exclusions)) continue;

				const language = detectLanguage(filePath);
				if (!language) continue;

				try {
					const content = await Deno.readTextFile(filePath);
					files.push({
						path: filePath,
						content,
						language,
						bytes: new TextEncoder().encode(content).length,
					});
				} catch (e) {
					console.warn(`Warning: Could not read ${filePath}: ${e}`);
				}
			}
		}

		logger(`  Found ${files.length} files`);

		const stats = computeStats(files, path);
		logStats(stats, logger);

		return { files, stats };
	}
}
