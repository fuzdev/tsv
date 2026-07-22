import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the rest
	let rest = { id: 'x' };
	$$renderer.push(`<div${$.attributes({ ...rest })}>hi</div>`);
}
