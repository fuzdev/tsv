import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	var $$store_subs;
	let { count } = $$props;
	$$renderer.push(`<p>${$.escape($.store_get(($$store_subs ??= {}), '$count', count))}</p>`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
