import * as $ from 'svelte/internal/server';
import { writable } from 'svelte/store';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		const count = writable(0);
		class C {
			x = $.store_get(($$store_subs ??= {}), '$count', count) + 1;
		}
		const c = new C();
		$$renderer.push(`<p>${$.escape(c.x)}</p>`);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
