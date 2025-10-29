import { parse as parseSvelte } from 'svelte/compiler';
import * as acorn from 'acorn';
import { tsPlugin } from '@sveltejs/acorn-typescript';
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Create acorn parser with TypeScript support, exactly like Svelte does
const ParserWithTS = acorn.Parser.extend(tsPlugin());

/**
 * Generate expected.json for test fixtures by parsing input files with Svelte's compiler
 *
 * Supports both .svelte and .ts files:
 *   fixtures/1-integration/proof-of-concept/input.svelte
 *   fixtures/2-typescript-parser/literal/input.ts
 *   fixtures/3-svelte-parser/script-tag/input.svelte
 *
 * Usage:
 *   node tests/generate-expected-fixtures.js                    # Generate all fixtures
 *   node tests/generate-expected-fixtures.js --list             # List all fixtures
 *   node tests/generate-expected-fixtures.js 1-integration      # Generate category (prefix match)
 *   node tests/generate-expected-fixtures.js 2-typescript-parser/literal # Generate specific fixture
 *   node tests/generate-expected-fixtures.js literal            # Generate all matching "literal" (substring)
 *   node tests/generate-expected-fixtures.js typescript const   # Generate matching all terms
 */

/**
 * @param {string} fixturePath
 * @param {string} relativePath
 * @param {string} inputFile
 */
/**
 * @param {string} fixturePath
 * @param {string} relativePath
 * @param {string} inputFile
 */
function generateExpectedFixture(fixturePath, relativePath, inputFile) {
  const inputPath = join(fixturePath, inputFile);
  const expectedPath = join(fixturePath, 'expected.json');

  try {
    const source = readFileSync(inputPath, 'utf-8');
    const isTypeScript = inputFile === 'input.ts';

    /** @type {any} */
    let ast;

    if (isTypeScript) {
      // Parse TypeScript directly with acorn, exactly like Svelte does
      // Options match svelte/src/compiler/phases/1-parse/acorn.js
      ast = ParserWithTS.parse(source, {
        sourceType: 'module',
        ecmaVersion: 16,
        locations: true
      });
    } else {
      // Parse Svelte files with Svelte's parser
      ast = parseSvelte(source, { modern: true });
    }

    const json = JSON.stringify(ast, null, 2);
    writeFileSync(expectedPath, json + '\n', 'utf-8');
    console.log(`✓ Generated ${relativePath}/expected.json`);
  } catch (error) {
    console.error(`✗ Failed to generate ${relativePath}:`, error.message);
  }
}

/**
 * @param {string} dir
 * @param {string} base
 * @returns {Generator<{path: string, relative: string, inputFile: string}>}
 */
function* walkFixtures(dir, base = '') {
  const entries = readdirSync(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    const relativePath = base ? join(base, entry.name) : entry.name;

    if (entry.isDirectory()) {
      // Check for input files (.svelte or .ts)
      for (const inputFile of ['input.svelte', 'input.ts']) {
        const inputPath = join(fullPath, inputFile);
        try {
          statSync(inputPath);
          yield { path: fullPath, relative: relativePath, inputFile };
          break; // Found an input file, don't check others
        } catch {
          // File doesn't exist, try next
        }
      }

      // If no input file found, recurse into subdirectories
      if (!['input.svelte', 'input.ts'].some(f => {
        try { statSync(join(fullPath, f)); return true; } catch { return false; }
      })) {
        yield* walkFixtures(fullPath, relativePath);
      }
    }
  }
}

const args = process.argv.slice(2);
const listOnly = args.includes('--list');
const filters = args.filter(arg => arg !== '--list');

const fixturesDir = join(__dirname, 'fixtures');
let fixtures = Array.from(walkFixtures(fixturesDir));

// Apply filters: match if path contains ALL filter terms
if (filters.length > 0) {
  fixtures = fixtures.filter(({ relative }) => {
    const lowerPath = relative.toLowerCase();
    return filters.every(filter => lowerPath.includes(filter.toLowerCase()));
  });
}

if (fixtures.length === 0) {
  console.error('No fixtures found' + (filters.length > 0 ? ` matching: ${filters.join(' ')}` : ''));
  process.exit(1);
}

if (listOnly) {
  console.log('Found fixtures:');
  for (const { relative, inputFile } of fixtures) {
    console.log(`  ${relative} (${inputFile})`);
  }
  console.log(`\nTotal: ${fixtures.length}`);
} else {
  for (const { path, relative, inputFile } of fixtures) {
    generateExpectedFixture(path, relative, inputFile);
  }
}
