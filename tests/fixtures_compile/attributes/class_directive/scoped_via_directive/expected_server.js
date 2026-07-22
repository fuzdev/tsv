import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	$$renderer.push(`<div${$.attr_class('svelte-tsvhash', void 0, { active: x })}>text</div>`);
}
