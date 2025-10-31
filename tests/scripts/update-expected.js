import {parse as parseSvelte} from 'svelte/compiler';
import * as acorn from 'acorn';
import {tsPlugin} from '@sveltejs/acorn-typescript';
import {readFileSync, writeFileSync, readdirSync, statSync} from 'node:fs';
import {join} from 'node:path';
import {execSync} from 'node:child_process';

// Create acorn parser with TypeScript support, exactly like Svelte does
const ParserWithTS = acorn.Parser.extend(tsPlugin());

/**
 * Generate expected.json for test fixtures by parsing input files with Svelte's compiler
 *
 * Supports .svelte, .ts, and .css files:
 *   fixtures/typescript_parser/literal/input.ts
 *   fixtures/svelte_parser/script_tag/input.svelte
 *   fixtures/css_parser/simple_rule/input.css
 *
 * Usage:
 *   npm run fixtures:update-expected                                # generate all fixtures
 *   npm run fixtures:update-expected -- --list                      # list all fixtures
 *   npm run fixtures:update-expected -- 1_integration               # generate category (prefix match)
 *   npm run fixtures:update-expected -- 2_typescript_parser/literal # generate specific fixture
 *   npm run fixtures:update-expected -- literal                     # generate all matching "literal" (substring)
 *   npm run fixtures:update-expected -- typescript const            # generate matching all terms
 */

/**
 * @param {string} fixturePath
 * @param {string} relativePath
 * @param {string} inputFile
 * @returns {{created: boolean, updated: boolean, unchanged: boolean}}
 */
function generateExpectedFixture(fixturePath, relativePath, inputFile) {
	const inputPath = join(fixturePath, inputFile);
	const expectedPath = join(fixturePath, 'expected.json');

	try {
		const source = readFileSync(inputPath, 'utf-8');
		const isTypeScript = inputFile === 'input.ts';
		const isCss = inputFile === 'input.css';

		/** @type {any} */
		let ast;

		if (isTypeScript) {
			// Parse TypeScript directly with acorn, exactly like Svelte does
			// Options match svelte/src/compiler/phases/1-parse/acorn.js
			ast = ParserWithTS.parse(source, {
				sourceType: 'module',
				ecmaVersion: 16,
				locations: true,
			});
		} else if (isCss) {
			// For CSS files, use our Rust parser via the tsv CLI
			try {
				const output = execSync(`cargo run --quiet parse "${inputPath}" --pretty`, {
					encoding: 'utf-8',
				});
				ast = JSON.parse(output);
			} catch (e) {
				// Fallback: create a minimal structure for CSS
				console.warn(
					`⚠ Could not generate CSS AST with cargo run, using placeholder for ${relativePath}`,
				);
				ast = {css: {type: 'StyleSheet', children: []}, type: 'Root'};
			}
		} else {
			// Parse Svelte files with Svelte's parser
			ast = parseSvelte(source, {modern: true});
		}

		const json = JSON.stringify(ast, null, 2) + '\n';

		// Check if expected.json already exists and compare content
		let existingContent;
		try {
			existingContent = readFileSync(expectedPath, 'utf-8');
		} catch {
			existingContent = null;
		}

		if (json === existingContent) {
			// File exists and content is identical
			console.log(`- ${relativePath}/expected.json is up to date`);
			return {created: false, updated: false, unchanged: true};
		} else if (existingContent === null) {
			// File doesn't exist, create it
			writeFileSync(expectedPath, json, 'utf-8');
			console.log(`✓ Created ${relativePath}/expected.json`);
			return {created: true, updated: false, unchanged: false};
		} else {
			// File exists but content differs, update it
			writeFileSync(expectedPath, json, 'utf-8');
			console.log(`✓ Updated ${relativePath}/expected.json`);
			return {created: false, updated: true, unchanged: false};
		}
	} catch (error) {
		console.error(`✗ Failed to generate ${relativePath}:`, error.message);
		return {created: false, updated: false, unchanged: false};
	}
}

/**
 * @param {string} dir
 * @param {string} base
 * @param {string} rootDir
 * @returns {Generator<{path: string, relative: string, inputFile: string}>}
 */
function* walkFixtures(dir, base = '', rootDir = null) {
	// Track the root directory for ./ prefix
	if (rootDir === null) {
		rootDir = dir;
	}

	const entries = readdirSync(dir, {withFileTypes: true});

	for (const entry of entries) {
		const fullPath = join(dir, entry.name);
		const relativePath = base ? join(base, entry.name) : entry.name;

		if (entry.isDirectory()) {
			// Check for input files (.svelte, .ts, or .css)
			for (const inputFile of ['input.svelte', 'input.ts', 'input.css']) {
				const inputPath = join(fullPath, inputFile);
				try {
					statSync(inputPath);
					// Return path with ./ prefix
					const displayPath = './' + join(rootDir, relativePath);
					yield {path: fullPath, relative: displayPath, inputFile};
					break; // Found an input file, don't check others
				} catch {
					// File doesn't exist, try next
				}
			}

			// If no input file found, recurse into subdirectories
			if (
				!['input.svelte', 'input.ts', 'input.css'].some((f) => {
					try {
						statSync(join(fullPath, f));
						return true;
					} catch {
						return false;
					}
				})
			) {
				yield* walkFixtures(fullPath, relativePath, rootDir);
			}
		}
	}
}

const args = process.argv.slice(2);
const listOnly = args.includes('--list');
const filters = args.filter((arg) => arg !== '--list');

const fixturesDir = 'tests/fixtures';
let fixtures = Array.from(walkFixtures(fixturesDir));

// Apply filters: match if path contains ALL filter terms
if (filters.length > 0) {
	fixtures = fixtures.filter(({relative}) => {
		const lowerPath = relative.toLowerCase();
		return filters.every((filter) => lowerPath.includes(filter.toLowerCase()));
	});
}

if (fixtures.length === 0) {
	console.error(
		'No fixtures found' + (filters.length > 0 ? ` matching: ${filters.join(' ')}` : ''),
	);
	process.exit(1);
}

if (listOnly) {
	console.log('Found fixtures:');
	for (const {relative, inputFile} of fixtures) {
		console.log(`  ${relative} (${inputFile})`);
	}
	console.log(`\nTotal: ${fixtures.length}`);
} else {
	let created = 0;
	let updated = 0;
	let unchanged = 0;

	for (const {path, relative, inputFile} of fixtures) {
		const result = generateExpectedFixture(path, relative, inputFile);
		if (result.created) created++;
		if (result.updated) updated++;
		if (result.unchanged) unchanged++;
	}

	console.log(
		`\nSummary: ${created} created, ${updated} updated, ${unchanged} unchanged (total: ${fixtures.length})`,
	);

	if (created > 0 || updated > 0) {
		console.log('⚠️  Updated source of truth files (expected.json)');
	}
}
