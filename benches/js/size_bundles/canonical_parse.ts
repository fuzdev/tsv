/**
 * Size-bundle entry: the canonical PARSERS — the three the parse rows run and tsv is
 * a drop-in for: `svelte/compiler`'s `parse` + `parseCss`, and acorn extended with
 * `@sveltejs/acorn-typescript`. Prettier is not here on purpose: it exposes no
 * public parse API, and its internal ASTs are not the product the parse rows compare.
 *
 * Never executed by the harness — see `canonical_format.ts`.
 */

import { Parser } from 'acorn';
import { tsPlugin } from '@sveltejs/acorn-typescript';
import { parse, parseCss } from 'svelte/compiler';

const ts_parser = Parser.extend(tsPlugin() as any);

export const parse_svelte = (source: string): unknown => parse(source, { modern: true });

export const parse_css = (source: string): unknown => parseCss(source);

export const parse_typescript = (source: string): unknown =>
	ts_parser.parse(source, { sourceType: 'module', ecmaVersion: 'latest', locations: true });
