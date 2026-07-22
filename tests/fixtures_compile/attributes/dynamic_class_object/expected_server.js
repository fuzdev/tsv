import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = `foo`;
	let on = true;
	$$renderer.push(
		`<div${$.attr_class($.clsx({ active: on }))}></div> <div${$.attr_class($.clsx([a, `b`]))}></div>`
	);
}
