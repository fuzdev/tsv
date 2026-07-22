import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { $$slots, $$events, ...o } = $$props;
	let $$derived_array = $.derived(() => $.to_array(o, 2)),
		a = $.derived(() => $$derived_array()[0]),
		b = $.derived(() => $$derived_array()[1]);
	let $$derived_array_1 = $.derived(() => $.to_array(o, 2)),
		c = $.derived(() => $$derived_array_1()[0]),
		d = $.derived(() => $$derived_array_1()[1]);
	$$renderer.push(`<!---->${$.escape(a())}${$.escape(b())}${$.escape(c())}${$.escape(d())}`);
}
