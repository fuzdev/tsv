import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	$$renderer.push(`<div${$.attr_class('c', void 0, { x: x })} id="a">text</div>`);
}
