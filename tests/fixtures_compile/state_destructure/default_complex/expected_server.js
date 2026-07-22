import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	function f() {}
	let tmp = obj,
		a = $.fallback(tmp.a, f, true);
	$$renderer.push(`<!---->${$.escape(a)}`);
}
