import * as $ from 'svelte/internal/server';
import { fn } from './stores.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		function f() {
			return $.store_get(($$store_subs ??= {}), '$fn', fn)();
		}
		$$renderer.push(`<!---->${$.escape(f())}`);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
