import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the color
	let color = 'red';
	$$renderer.push(`<div${$.attr_style('', { color })}>hi</div>`);
}
