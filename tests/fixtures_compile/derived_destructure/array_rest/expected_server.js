import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let $$derived_array = $.derived(() => $.to_array(o)),
		a = $.derived(() => $$derived_array()[0]),
		rest = $.derived(() => $$derived_array().slice(1));
	$$renderer.push(`<!---->${$.escape(a())}${$.escape(rest())}`);
}
