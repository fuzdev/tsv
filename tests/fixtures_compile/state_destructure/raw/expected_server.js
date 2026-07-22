import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = {};
	let tmp = obj,
		a = tmp.a;
	$$renderer.push(`<!---->${$.escape(a)}`);
}
