import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let x = 1;
	$$renderer.push(`<div${$.attributes({ ...props }, void 0, { a: x })}></div>`);
}
