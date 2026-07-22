import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	$$renderer.push(`<div id="a" title="b"${$.attr_class('', void 0, { x: x })}>text</div>`);
}
