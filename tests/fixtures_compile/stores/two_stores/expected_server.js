import * as $ from 'svelte/internal/server';
import { a, b } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	$$renderer.push(
		`<p>${$.escape($.store_get(($$store_subs ??= {}), '$a', a))}${$.escape($.store_get(($$store_subs ??= {}), '$b', b))}</p>`
	);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
