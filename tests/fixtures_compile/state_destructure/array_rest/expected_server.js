import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let arr = [];
	let tmp = arr,
		$$array = $.to_array(tmp),
		a = $$array[0],
		rest = $$array.slice(1);
	$$renderer.push(`<!---->${$.escape(a)}
${$.escape(rest)}`);
}
