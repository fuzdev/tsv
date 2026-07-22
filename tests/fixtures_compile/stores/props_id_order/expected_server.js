import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	const id = $.props_id($$renderer);
	var $$store_subs;
	$$renderer.push(
		`<p${$.attr('id', id)}>${$.escape($.store_get(($$store_subs ??= {}), '$count', count))}</p>`
	);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
