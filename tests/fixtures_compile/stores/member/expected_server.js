import * as $ from 'svelte/internal/server';
import { store } from './stores.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		$$renderer.push(`<p>${$.escape($.store_get(($$store_subs ??= {}), '$store', store).foo)}</p>`);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
