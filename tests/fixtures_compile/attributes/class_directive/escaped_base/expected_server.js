import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	$$renderer.push(`<div${$.attr_class('a&amp;b&lt;c', void 0, { x: x })}>hi</div>`);
}
