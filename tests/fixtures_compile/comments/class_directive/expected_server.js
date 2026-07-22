import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// is active
	let active = true;
	$$renderer.push(`<div${$.attr_class('', void 0, { active: active })}>hi</div>`);
}
