import {readFile, readdir, stat} from 'node:fs/promises';
import {join} from 'node:path';
import {format, resolveConfig} from 'prettier';

const prettierConfig = await resolveConfig(import.meta.filename);

/**
 * Check that all formatted.* files are up to date with prettier output (CI validation)
 *
 * For each input.{svelte,ts,css}:
 * - Format with prettier
 * - If formatted.* exists: verify it matches prettier output
 * - If formatted.* doesn't exist: verify input already matches prettier output
 *
 * Exits with error code 1 if any mismatches found.
 *
 * Usage:
 *   npm run fixtures:check-expected              # check all fixtures
 *   npm run fixtures:check-expected -- literal   # check matching "literal"
 */

/**
 * @param {string} fixturePath
 * @param {string} relativePath
 * @param {string} inputFile
 * @returns {Promise<boolean>} true if valid, false if invalid
 */
async function checkFixture(fixturePath, relativePath, inputFile) {
	const inputPath = join(fixturePath, inputFile);
	const formattedFile = inputFile.replace('input.', 'formatted.');
	const formattedPath = join(fixturePath, formattedFile);

	try {
		const input = await readFile(inputPath, 'utf-8');

		// Format with prettier
		const prettierOutput = await format(input, {
			...prettierConfig,
			filepath: inputPath,
		});

		// Check if formatted.* exists
		const formattedExists = await (async () => {
			try {
				await stat(formattedPath);
				return true;
			} catch {
				return false;
			}
		})();

		if (formattedExists) {
			// formatted.* exists - verify it matches prettier output
			const formatted = await readFile(formattedPath, 'utf-8');

			if (formatted !== prettierOutput) {
				console.error(
					`✗ ${relativePath}/${formattedFile} is out of date. Run: npm run fixtures:update-formatted`,
				);
				return false;
			}

			// Check if formatted.* should actually exist (input might now match prettier)
			if (prettierOutput === input) {
				console.error(
					`✗ ${relativePath}/${formattedFile} should not exist (input is now formatted). Run: npm run fixtures:update-formatted`,
				);
				return false;
			}

			console.log(`✓ ${relativePath}/${formattedFile} is up to date`);
			return true;
		} else {
			// formatted.* doesn't exist - verify input matches prettier output
			if (prettierOutput !== input) {
				console.error(
					`✗ ${relativePath}/${inputFile} differs from prettier output but no formatted.* exists. Run: npm run fixtures:update-formatted`,
				);
				return false;
			}

			console.log(`✓ ${relativePath}/${inputFile} is already formatted`);
			return true;
		}
	} catch (error) {
		console.error(`✗ Failed to check ${relativePath}:`, error.message);
		return false;
	}
}

/**
 * @param {string} dir
 * @param {string} base
 * @returns {AsyncGenerator<{path: string, relative: string, inputFile: string}>}
 */
async function* walkFixtures(dir, base = '') {
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
					yield {path: fullPath, relative: relativePath, inputFile};
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
				yield* walkFixtures(fullPath, relativePath);
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

let valid = 0;
let invalid = 0;

for (const {path, relative, inputFile} of filteredFixtures) {
	if (await checkFixture(path, relative, inputFile)) {
		valid++;
	} else {
		invalid++;
	}
}

console.log(`\nSummary: ${valid} valid, ${invalid} invalid (total: ${filteredFixtures.length})`);

if (invalid > 0) {
	console.error('\nFormatted fixtures check failed. Run: npm run fixtures:update-formatted');
	process.exit(1);
}
