import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let $$derived_array = $.derived(() => $.to_array(o, 2)),
		a = $.derived(() => $$derived_array()[0]),
		b = $.derived(() => $$derived_array()[1]);
	$$renderer.push(`<!---->${$.escape(a())}${$.escape(b())}`);
}
