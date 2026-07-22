import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	let b = $.derived(() => a * 2);
	// note after derived
	let c = 3;
	$$renderer.push(`<p>23</p>`);
}
