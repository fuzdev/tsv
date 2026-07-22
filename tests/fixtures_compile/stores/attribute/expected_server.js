import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	$$renderer.push(`<p${$.attr('id', $.store_get(($$store_subs ??= {}), '$count', count))}>x</p>`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
