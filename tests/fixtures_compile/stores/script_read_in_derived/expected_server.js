import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	let doubled = $.derived(() => $.store_get(($$store_subs ??= {}), '$count', count) * 2);
	$$renderer.push(`<p>${$.escape(doubled())}</p>`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
