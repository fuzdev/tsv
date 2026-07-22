import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	let tmp = obj,
		a = tmp.a,
		b = tmp.b;
	$$renderer.push(`<!---->${$.escape(a)}${$.escape(b)}`);
}
