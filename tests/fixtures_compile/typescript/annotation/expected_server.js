import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = 1;
	let label = 'hi';
	$$renderer.push(`<p>1hi</p>`);
}
