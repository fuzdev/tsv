import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let tmp = [1, 2],
		$$array = $.to_array(tmp, 2),
		a = $$array[0],
		b = $$array[1];
	$$renderer.push(`<!---->${$.escape(a)}${$.escape(b)}`);
}
