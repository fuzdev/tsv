/**
 * Compare our formatter output with prettier
 * Usage: node tests/debug/compare.js <file.svelte>
 *        node tests/debug/compare.js --content "<div>content</div>"
 */

import {execSync} from 'child_process';
import {readFileSync, existsSync} from 'fs';
import {resolve} from 'path';
import {format, resolveConfig} from 'prettier';

const prettierConfig = await resolveConfig(import.meta.filename);

const [firstArg, secondArg] = process.argv.slice(2);

if (!firstArg) {
	console.error('Usage: node tests/debug/compare.js <file.svelte>');
	console.error('   OR: node tests/debug/compare.js --content "<div>content</div>"');
	console.error('Example: node tests/debug/compare.js tests/debug/expression_spacing.svelte');
	process.exit(1);
}

let input;
let file;
let parser;

if (firstArg === '--content') {
	// Inline content mode
	input = secondArg;
	if (!input) {
		console.error('Error: --content requires a content argument');
		process.exit(1);
	}
	// Default to svelte parser
	file = 'temp.svelte';
	parser = 'svelte';
} else {
	// File path mode
	file = resolve(firstArg);
	if (!existsSync(file)) {
		console.error(`Error: File not found: ${file}`);
		process.exit(1);
	}
	input = readFileSync(file, 'utf8');

	// Detect parser from extension
	if (file.endsWith('.svelte')) {
		parser = 'svelte';
	} else if (file.endsWith('.css')) {
		parser = 'css';
	} else {
		parser = 'typescript';
	}
}

console.log('=== Input ===');
console.log(input);
console.log();

console.log('=== Our Formatter ===');
try {
	const ourOutput = execSync(`cargo run --quiet format --stdin --parser ${parser}`, {
		input,
		encoding: 'utf8',
		stdio: ['pipe', 'pipe', 'pipe'],
	});
	console.log(ourOutput);
} catch (err) {
	console.error('Error running our formatter:', err.message);
}
console.log();

console.log('=== Prettier ===');
try {
	const prettierOutput = await format(input, {
		...prettierConfig,
		filepath: file,
	});
	console.log(prettierOutput);
} catch (err) {
	console.error('Error running prettier:', err.message);
}
