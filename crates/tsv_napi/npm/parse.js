/**
 * `@fuzdev/tsv/parse` — the parse family over the native addon: `parse_<lang>` (the
 * span-only AST as objects, `{locations: true}` adding `loc`) and the `locations.js`
 * reconstruction helpers. The same entry, and the same names, as `@fuzdev/tsv-wasm/parse`.
 *
 * `parse_<lang>` takes the tape route: the addon's raw `parse_tape_<lang>` lays the wire down
 * as words, copied into a buffer the JS engine owns, and `tape_engine.js` decodes it with the
 * generated decoder (`tape_decode.js`) into the tree `JSON.parse` would make of the wire — so
 * this entry's static graph holds the decoder, and the root (`index.js`) does not.
 *
 * The addon comes from `addon.js`, the module the root imports too, so it loads once.
 */

import { addon } from './addon.js';
import { create_parse_api } from './api_parse.js';
import { create_tape_parse } from './tape_engine.js';

const from_addon = (name) =>
	Object.fromEntries(
		['svelte', 'typescript', 'css'].map((language) => [
			language,
			addon[name.replace('<lang>', language)]
		])
	);

export const { parse_svelte, parse_typescript, parse_css } = create_parse_api({
	parse: create_tape_parse(from_addon('parse_tape_<lang>'))
});

// The reconstruction helpers — pure JS over the span-only wire, no addon — staged
// beside this file from `crates/tsv_wasm/npm/`, the wasm packages' copy verbatim.
export * from './locations.js';
