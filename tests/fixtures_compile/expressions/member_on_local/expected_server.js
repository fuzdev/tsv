import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = { b: 1 };
	$$renderer.push(`<p>${$.escape(a.b)}</p>`);
}
