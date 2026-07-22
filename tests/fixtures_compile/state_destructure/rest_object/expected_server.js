import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	let tmp = obj,
		a = tmp.a,
		rest = $.exclude_from_object(tmp, ['a']);
	$$renderer.push(`<!---->${$.escape(a)}`);
}
