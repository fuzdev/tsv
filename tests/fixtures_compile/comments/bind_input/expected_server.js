import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the value
	let value = '';
	$$renderer.push(`<input${$.attr('value', value)}/>`);
}
