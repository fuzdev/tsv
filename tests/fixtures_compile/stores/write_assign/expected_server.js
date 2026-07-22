import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	function set_five() {
		$.store_set(count, 5);
	}
	$$renderer.push(
		`<button>${$.escape($.store_get(($$store_subs ??= {}), '$count', count))}</button>`
	);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
