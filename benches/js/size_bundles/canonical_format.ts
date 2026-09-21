/**
 * Size-bundle entry: the canonical FORMATTER, reduced to what formatting tsv's three
 * languages needs — `prettier/standalone` plus the estree / typescript / babel /
 * postcss plugins and `prettier-plugin-svelte`'s browser build (which pulls in
 * `svelte/compiler`'s parser). babel stays in: the svelte plugin parses template
 * expressions and plain-JS `<script>` blocks through it.
 *
 * Never executed by the harness — `lib/canonical_bundles.ts` bundles it and
 * `lib/binary_sizes.ts` sizes the result. The exports exist so the bundler keeps
 * the code a caller would reach; nothing here is a measured code path.
 */

import type { Plugin } from 'prettier';
import { format } from 'prettier/standalone';
import * as estree from 'prettier/plugins/estree';
import * as typescript from 'prettier/plugins/typescript';
import * as babel from 'prettier/plugins/babel';
import * as postcss from 'prettier/plugins/postcss';
import * as svelte from 'prettier-plugin-svelte/browser';

// The browser build ships no types; its inferred option shapes are not `SupportOption`s.
const plugins: Plugin[] = [estree, typescript, babel, postcss, svelte as unknown as Plugin];

export const format_source = (source: string, parser: string): Promise<string> =>
	format(source, { parser, plugins });
