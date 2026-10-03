/**
 * The README fence and marker grammar `typecheck_packages.ts` reads: which blocks it
 * grades, under which marker, and the authoring mistakes it refuses rather than
 * silently grading nothing.
 */

import { deepStrictEqual, throws } from 'node:assert';

import { readme_blocks } from './typecheck_packages.ts';

Deno.test('a fence with an info string after its language opens a graded block', () => {
	const markdown = [
		'```ts title="a.ts"',
		'const a = 1;',
		'```',
		'```javascript',
		'b();',
		'```'
	].join('\n');
	deepStrictEqual(readme_blocks(markdown, 'R'), [
		{ line: 1, lang: 'ts', code: 'const a = 1;', node: false },
		{ line: 4, lang: 'js', code: 'b();', node: false }
	]);
});

Deno.test('an ungraded fence stays paired, so the block after it is still read', () => {
	const markdown = ['```bash title="run"', '```ts', '```', '```typescript', 'c;', '```'].join('\n');
	deepStrictEqual(readme_blocks(markdown, 'R'), [{ line: 4, lang: 'ts', code: 'c;', node: false }]);
});

Deno.test('a node marker flags only the block directly below it', () => {
	const markdown = ['<!-- typecheck: node -->', '```ts', 'a;', '```', '```ts', 'b;', '```'].join(
		'\n'
	);
	deepStrictEqual(readme_blocks(markdown, 'R'), [
		{ line: 2, lang: 'ts', code: 'a;', node: true },
		{ line: 5, lang: 'ts', code: 'b;', node: false }
	]);
});

Deno.test('an unknown marker is refused', () => {
	throws(
		() => readme_blocks('<!-- typecheck: skip -->\n```ts\na;\n```', 'R'),
		/R:1: unknown marker/
	);
});

Deno.test('a marker not immediately followed by a graded fence is refused', () => {
	for (const markdown of [
		'<!-- typecheck: node -->\n\n```ts\na;\n```',
		'<!-- typecheck: node -->\n```bash\na\n```',
		'<!-- typecheck: node -->'
	]) {
		throws(() => readme_blocks(markdown, 'R'), /R:1: marker not immediately followed/);
	}
});

Deno.test('an unclosed fence is refused', () => {
	throws(() => readme_blocks('```ts\na;', 'R'), /R:1: unclosed fence/);
});
