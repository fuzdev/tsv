import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = { x: 1 };
	$$renderer.push(`<div>text</div>`);
}
