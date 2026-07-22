import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let tmp = { a: 1, b: 2 },
		a = tmp.a,
		b = tmp.b;
	$$renderer.push(`<!---->${$.escape(a)}${$.escape(b)}`);
}
