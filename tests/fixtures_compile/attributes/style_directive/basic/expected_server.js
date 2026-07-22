import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	let w = 1;
	$$renderer.push(`<div${$.attr_style('x', { color: w })}>text</div>`);
}
