import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let count = 0;
	$$renderer.push(`<button>inc</button>`);
}
