import {readFile, writeFile, readdir, stat, unlink} from 'node:fs/promises';
import {join} from 'node:path';
import {format, resolveConfig} from 'prettier';

const prettierConfig = await resolveConfig(import.meta.filename);

/**
 * Format all fixture input files with prettier and create formatted.* files only when output differs
 *
 * For each input.{svelte,ts,css}:
 * - Format with prettier
 * - If output differs from input: write formatted.{svelte,ts,css}
 * - If output matches input: delete formatted.{svelte,ts,css} (if it exists)
 *
 * Usage:
 *   npm run fixtures:update-formatted                       # format all fixtures
 *   npm run fixtures:update-formatted -- literal            # format matching "literal"
 *   npm run fixtures:update-formatted -- typescript const   # format matching all terms
 */

/**
 * @param {string} fixturePath
 * @param {string} relativePath
 * @param {string} inputFile
 * @returns {Promise<{created: boolean, updated: boolean, removed: boolean, unchanged: boolean}>}
 */
async function formatFixture(fixturePath, relativePath, inputFile) {
	const inputPath = join(fixturePath, inputFile);
	const formattedFile = inputFile.replace('input.', 'formatted.');
	const formattedPath = join(fixturePath, formattedFile);

	try {
		const input = await readFile(inputPath, 'utf-8');

		// Format with prettier
		const formatted = await format(input, {
			...prettierConfig,
			filepath: inputPath,
		});

		// Check if formatted differs from input
		if (formatted === input) {
			// Input is already formatted - remove formatted.* if it exists
			try {
				await stat(formattedPath);
				await unlink(formattedPath);
				console.log(`✓ Removed ${relativePath}/${formattedFile} (input is already formatted)`);
				return {created: false, updated: false, removed: true, unchanged: false};
			} catch {
				// formatted.* doesn't exist, nothing to do
				return {created: false, updated: false, removed: false, unchanged: true};
			}
		} else {
			// Input differs from formatted output - check if we need to write formatted.*
			let existingFormatted;
			try {
				existingFormatted = await readFile(formattedPath, 'utf-8');
			} catch {
				existingFormatted = null;
			}

			// Only write if content differs or file doesn't exist
			if (formatted === existingFormatted) {
				// File exists and is already correct
				console.log(`- ${relativePath}/${formattedFile} is up to date`);
				return {created: false, updated: false, removed: false, unchanged: true};
			} else if (existingFormatted === null) {
				// File doesn't exist, create it
				await writeFile(formattedPath, formatted, 'utf-8');
				console.log(`✓ Created ${relativePath}/${formattedFile}`);
				return {created: true, updated: false, removed: false, unchanged: false};
			} else {
				// File exists but content differs, update it
				await writeFile(formattedPath, formatted, 'utf-8');
				console.log(`✓ Updated ${relativePath}/${formattedFile}`);
				return {created: false, updated: true, removed: false, unchanged: false};
			}
		}
	} catch (error) {
		console.error(`✗ Failed to format ${relativePath}:`, error.message);
		return {created: false, updated: false, removed: false, unchanged: false};
	}
}

/**
 * @param {string} dir
 * @param {string} base
 * @param {string} rootDir
 * @returns {AsyncGenerator<{path: string, relative: string, inputFile: string}>}
 */
async function* walkFixtures(dir, base = '', rootDir = null) {
	// Track the root directory for ./ prefix
	if (rootDir === null) {
		rootDir = dir;
	}

	const entries = await readdir(dir, {withFileTypes: true});

	for (const entry of entries) {
		const fullPath = join(dir, entry.name);
		const relativePath = base ? join(base, entry.name) : entry.name;

		if (entry.isDirectory()) {
			// Check for input files (.svelte, .ts, or .css)
			for (const inputFile of ['input.svelte', 'input.ts', 'input.css']) {
				const inputPath = join(fullPath, inputFile);
				try {
					await stat(inputPath);
					// Return path with ./ prefix
					const displayPath = './' + join(rootDir, relativePath);
					yield {path: fullPath, relative: displayPath, inputFile};
					break; // Found an input file, don't check others
				} catch {
					// File doesn't exist, try next
				}
			}

			// If no input file found, recurse into subdirectories
			const hasInput = await (async () => {
				for (const f of ['input.svelte', 'input.ts', 'input.css']) {
					try {
						await stat(join(fullPath, f));
						return true;
					} catch {
						// continue
					}
				}
				return false;
			})();

			if (!hasInput) {
				yield* walkFixtures(fullPath, relativePath, rootDir);
			}
		}
	}
}

const args = process.argv.slice(2);
const filters = args;

const fixturesDir = 'tests/fixtures';
const fixtures = [];
for await (const fixture of walkFixtures(fixturesDir)) {
	fixtures.push(fixture);
}

// Apply filters: match if path contains ALL filter terms
const filteredFixtures =
	filters.length > 0
		? fixtures.filter(({relative}) => {
				const lowerPath = relative.toLowerCase();
				return filters.every((filter) => lowerPath.includes(filter.toLowerCase()));
			})
		: fixtures;

if (filteredFixtures.length === 0) {
	console.error(
		'No fixtures found' + (filters.length > 0 ? ` matching: ${filters.join(' ')}` : ''),
	);
	process.exit(1);
}

let created = 0;
let updated = 0;
let removed = 0;
let unchanged = 0;

for (const {path, relative, inputFile} of filteredFixtures) {
	const result = await formatFixture(path, relative, inputFile);
	if (result.created) created++;
	if (result.updated) updated++;
	if (result.removed) removed++;
	if (result.unchanged) unchanged++;
}

console.log(
	`\nSummary: ${created} created, ${updated} updated, ${removed} removed, ${unchanged} unchanged (total: ${filteredFixtures.length})`,
);

if (created > 0 || updated > 0 || removed > 0) {
	console.log('⚠️  Updated source of truth files (formatted.*)');
}
