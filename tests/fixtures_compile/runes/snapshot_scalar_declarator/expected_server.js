import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	let b = a;
	$$renderer.push(`<p>${$.escape(b)}</p>`);
}
