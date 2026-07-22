import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	let b = a + 1;
	$$renderer.push(`<p>2</p>`);
}
