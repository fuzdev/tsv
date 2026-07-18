import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	if ($.store_get(($$store_subs ??= {}), '$count', count)) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>yes</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]-->`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
