import * as $ from 'svelte/internal/server';
import { C } from './stores.js';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		var $$store_subs;
		function f() {
			return new ($.store_get(($$store_subs ??= {}), '$C', C))();
		}
		$$renderer.push(`<!---->${$.escape(f())}`);
		if ($$store_subs) $.unsubscribe_stores($$store_subs);
	});
}
