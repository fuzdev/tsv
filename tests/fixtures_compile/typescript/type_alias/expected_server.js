import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 'x';
	$$renderer.push(`<p>x</p>`);
}
