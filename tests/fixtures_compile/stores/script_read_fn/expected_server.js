import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	function total() {
		return $.store_get(($$store_subs ??= {}), '$count', count) + 1;
	}
	$$renderer.push(`<p>${$.escape(total())}</p>`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
