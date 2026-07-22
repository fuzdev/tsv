import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the class
	let c = 'active';
	$$renderer.push(`<div${$.attr_class($.clsx(c))}>hi</div>`);
}
