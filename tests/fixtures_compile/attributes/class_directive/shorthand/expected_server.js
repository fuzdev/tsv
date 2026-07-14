import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let active = true;
	$$renderer.push(`<div${$.attr_class('', void 0, { active: active })}>text</div>`);
}
