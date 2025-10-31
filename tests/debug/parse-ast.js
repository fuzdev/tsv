/**
 * Parse a Svelte file and pretty-print the AST
 * Usage: node tests/debug/parse-ast.js <file.svelte>
 */

import {execSync} from 'child_process';
import {existsSync} from 'fs';
import {resolve} from 'path';

const [inputFile] = process.argv.slice(2);

if (!inputFile) {
	console.error('Usage: node tests/debug/parse-ast.js <file.svelte>');
	console.error('Example: node tests/debug/parse-ast.js tests/debug/expression_spacing.svelte');
	process.exit(1);
}

const file = resolve(inputFile);

if (!existsSync(file)) {
	console.error(`Error: File not found: ${file}`);
	process.exit(1);
}

console.log(`=== AST for ${file} ===`);
console.log();

try {
	const ast = execSync(`cargo run --quiet parse "${file}" --pretty`, {
		encoding: 'utf8',
		stdio: ['pipe', 'pipe', 'pipe'],
	});
	console.log(ast);
} catch (err) {
	console.error('Error parsing:', err.message);
	process.exit(1);
}
