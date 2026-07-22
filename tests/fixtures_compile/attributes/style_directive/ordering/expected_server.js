import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	let w = 1;
	$$renderer.push(`<div id="a"${$.attr_style('color:red', { margin: w })} title="b">text</div>`);
}
