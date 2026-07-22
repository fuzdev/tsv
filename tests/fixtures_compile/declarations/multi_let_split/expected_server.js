import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = { b: 1 };
	let b = 0;
	$$renderer.push(`<p>${$.escape(a.b)}</p>`);
}
