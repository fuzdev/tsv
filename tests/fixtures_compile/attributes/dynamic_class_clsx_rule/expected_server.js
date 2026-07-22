import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = `foo`;
	let b = `bar`;
	$$renderer.push(
		`<div${$.attr_class($.clsx(a))}></div> <div${$.attr_class(a + ` x`)}></div> <div${$.attr_class(`p ${b}`)}></div>`
	);
}
