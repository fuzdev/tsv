/**
 * Corpus loading for benchmarks.
 *
 * Provides a pluggable interface for loading source files from different sources.
 * The default implementation loads from ~/dev/ repos.
 */

import { walk } from '@std/fs/walk';
import { basename, extname } from '@std/path';

import type { CorpusStats, Language, Logger, SourceFile } from './types.ts';

export type { Logger };

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

// ============================================================================
// Shared Utilities
// ============================================================================

/** Detect language from file extension */
export function detectLanguage(path: string): Language | null {
	const ext = extname(path).toLowerCase();
	switch (ext) {
		case '.svelte':
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
	'.test.', // Test files
	'.spec.', // Spec files
	'/node_modules/',
	'/.svelte-kit/',
	'/build/',
	'/dist/',
];

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

// ============================================================================
// Default Implementation: ~/dev/ repos
// ============================================================================

/** Configuration for DevReposLoader */
export interface DevReposLoaderOptions {
	/** Base directory containing repos (default: ~/dev) */
	baseDir?: string;
	/** List of repo names to load (default: hardcoded list) */
	repos?: string[];
	/** Subdirectory within each repo to scan (default: src) */
	srcDir?: string;
	/** File extensions to include (default: svelte, ts, js, css) */
	extensions?: string[];
	/** Patterns to exclude (default: DEFAULT_EXCLUSIONS) */
	exclusions?: string[];
}

/** Default repos for the dev loader */
const DEFAULT_REPOS = [
	'zzz',
	'fuz_css',
	'fuz_ui',
	'gro',
	'fuz_util',
	'fuz_template',
	'fuz_blog',
	'fuz_mastodon',
	'fuz_code',
	'fuz_gitops',
	'webdevladder.net',
	'ryanatkn.com',
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

		for (const repoName of repos) {
			const repoPath = `${baseDir}/${repoName}`;
			const srcPath = `${repoPath}/${srcDir}`;

			try {
				await Deno.stat(srcPath);
			} catch {
				// No src directory or repo not found, skip
				continue;
			}

			const repoFiles = await this.#loadDirectory(srcPath, extensions, exclusions);
			if (repoFiles.length > 0) {
				allFiles.push(...repoFiles);
				loadedRepos.push(repoName);
				logger(`  ${repoName}: ${repoFiles.length} files`);
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

// ============================================================================
// Alternative Implementation: Single Directory
// ============================================================================

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
