import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = true;
	let w = 1;
	$$renderer.push(`<div${$.attr_class($.clsx(w), void 0, { active: x })}>text</div>`);
}
