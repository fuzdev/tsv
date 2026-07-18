import * as $ from 'svelte/internal/server';
import { obj } from './stores.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		function f() {
			return $.store_get(($$store_subs ??= {}), '$obj', obj).m();
		}
		$$renderer.push(`<!---->${$.escape(f())}`);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
