import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	// first
	let b = $.derived(() => a * 2);
	// second
	let c = $.derived(() => a + 1);
	// third
	let d = 3;
	$$renderer.push(`<p>223</p>`);
}
