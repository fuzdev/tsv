import * as $ from 'svelte/internal/server';
import { writable } from 'svelte/store';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		// the store
		const count = writable(1);
		let doubled = /* read */ $.store_get(($$store_subs ??= {}), '$count', count) * 2; // doubled
		function inc() {
			// bump it
			$.store_set(count, $.store_get(($$store_subs ??= {}), '$count', count) + /* step */ 1);
			$.update_store(($$store_subs ??= {}), '$count', count); // again
			$.store_set(count, /* reset */ 0);
		}
		$$renderer.push(
			`<button>${$.escape(doubled)} ${$.escape($.store_get(($$store_subs ??= {}), '$count', count))}</button>`
		);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
