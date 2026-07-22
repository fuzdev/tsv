import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	// note before derived
	let b = $.derived(() => a * 2);
	$$renderer.push(`<p>2</p>`);
}
