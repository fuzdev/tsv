import * as $ from 'svelte/internal/server';
import { writable } from 'svelte/store';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		let count = writable(0);
		let doubled = $.store_get(($$store_subs ??= {}), '$count', count) * 2;
		$$renderer.push(`<p>${$.escape(doubled)}</p>`);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
