import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	let tmp = obj,
		b = tmp.a.b;
	$$renderer.push(`<!---->${$.escape(b)}`);
}
