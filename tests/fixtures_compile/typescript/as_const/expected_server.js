import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = [1, 2];
	$$renderer.push(`<p>${$.escape(a.length)}</p>`);
}
