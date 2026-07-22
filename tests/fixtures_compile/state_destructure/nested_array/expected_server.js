import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	let tmp = obj,
		$$array = $.to_array(tmp.a, 2),
		b = $$array[0],
		c = $$array[1];
	$$renderer.push(`<!---->${$.escape(b)}${$.escape(c)}`);
}
