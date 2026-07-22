import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = { a: 1 };
	const s = obj;
	$$renderer.push(`<!---->${$.escape(s.a)}`);
}
