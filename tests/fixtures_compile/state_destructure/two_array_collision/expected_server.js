import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = [];
	let tmp = x,
		$$array = $.to_array(tmp, 2),
		a = $$array[0],
		b = $$array[1];
	let tmp_1 = x,
		$$array_1 = $.to_array(tmp_1, 2),
		c = $$array_1[0],
		d = $$array_1[1];
	$$renderer.push(`<!---->${$.escape(a)}${$.escape(b)}${$.escape(c)}${$.escape(d)}`);
}
