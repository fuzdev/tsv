import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	let tmp = obj,
		a = $.fallback(tmp.a, 1);
	$$renderer.push(`<!---->${$.escape(a)}`);
}
