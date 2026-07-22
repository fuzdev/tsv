import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let arr = [];
	let tmp = arr,
		$$array = $.to_array(tmp, 3),
		a = $$array[0],
		b = $$array[2];
	$$renderer.push(`<!---->${$.escape(a)}${$.escape(b)}`);
}
