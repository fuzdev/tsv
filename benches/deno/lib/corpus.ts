/**
 * Corpus loading for benchmarks
 *
 * Loads all .svelte, .ts, and .css files from src/ directories
 * of repos in ~/dev/
 */

import { walk } from 'jsr:@std/fs@1/walk';
import { basename, extname } from 'jsr:@std/path@1';
import type { CorpusStats, Language, SourceFile } from './types.ts';

/** Hardcoded list of repos to benchmark */
const REPOS = [
	'zzz',
	'moss',
	'fuz_ui',
	'gro',
	'belt',
	'fuz_template',
	'fuz_blog',
	'fuz_mastodon',
	'fuz_code',
	'fuz_gitops',
	'webdevladder.net',
	'ryanatkn.com',
];

/** Detect language from file extension */
function detectLanguage(path: string): Language | null {
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

/** Check if file should be excluded */
function shouldExclude(path: string): boolean {
	const name = basename(path);

	// Skip declaration files
	if (name.endsWith('.d.ts')) return true;

	// Skip test files for cleaner benchmarks
	if (name.includes('.test.') || name.includes('.spec.')) return true;

	// Skip generated files
	if (path.includes('/node_modules/')) return true;
	if (path.includes('/.svelte-kit/')) return true;
	if (path.includes('/build/')) return true;
	if (path.includes('/dist/')) return true;

	return false;
}

/** Load all source files from a single repo */
async function loadRepoFiles(
	repoPath: string,
): Promise<SourceFile[]> {
	const srcPath = `${repoPath}/src`;
	const files: SourceFile[] = [];

	try {
		// Check if src directory exists
		await Deno.stat(srcPath);
	} catch {
		// No src directory, skip this repo
		return files;
	}

	for await (
		const entry of walk(srcPath, {
			exts: ['svelte', 'ts', 'js', 'css'],
			includeDirs: false,
		})
	) {
		if (shouldExclude(entry.path)) continue;

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

/** Load the complete benchmark corpus */
export async function loadCorpus(): Promise<{
	files: SourceFile[];
	stats: CorpusStats;
}> {
	const homeDir = Deno.env.get('HOME');
	if (!homeDir) {
		throw new Error('HOME environment variable not set');
	}

	const devDir = `${homeDir}/dev`;

	console.log(`Loading ${REPOS.length} repos`);

	// Load files from all repos
	const allFiles: SourceFile[] = [];
	const loadedRepos: string[] = [];

	for (const repoName of REPOS) {
		const repoPath = `${devDir}/${repoName}`;

		try {
			await Deno.stat(repoPath);
		} catch {
			console.warn(`Warning: Repo not found at ${repoPath}, skipping`);
			continue;
		}

		const files = await loadRepoFiles(repoPath);
		if (files.length > 0) {
			allFiles.push(...files);
			loadedRepos.push(repoName);
			console.log(`  ${repoName}: ${files.length} files`);
		}
	}

	// Compute statistics
	const stats: CorpusStats = {
		totalFiles: allFiles.length,
		totalBytes: allFiles.reduce((sum, f) => sum + f.bytes, 0),
		byLanguage: {
			svelte: { files: 0, bytes: 0 },
			typescript: { files: 0, bytes: 0 },
			css: { files: 0, bytes: 0 },
		},
		repos: loadedRepos,
	};

	for (const file of allFiles) {
		stats.byLanguage[file.language].files++;
		stats.byLanguage[file.language].bytes += file.bytes;
	}

	console.log(`\nCorpus loaded:`);
	console.log(
		`  Total: ${stats.totalFiles} files, ${(stats.totalBytes / 1024 / 1024).toFixed(2)} MB`,
	);
	console.log(`  Svelte: ${stats.byLanguage.svelte.files} files`);
	console.log(`  TypeScript: ${stats.byLanguage.typescript.files} files`);
	console.log(`  CSS: ${stats.byLanguage.css.files} files`);

	return { files: allFiles, stats };
}

/** Group files by language for targeted benchmarks */
export function groupByLanguage(
	files: SourceFile[],
): Record<Language, SourceFile[]> {
	return {
		svelte: files.filter((f) => f.language === 'svelte'),
		typescript: files.filter((f) => f.language === 'typescript'),
		css: files.filter((f) => f.language === 'css'),
	};
}
