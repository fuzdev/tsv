import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the id part
	let x = 'foo';
	$$renderer.push(`<div title="a-foo-b">hi</div>`);
}
