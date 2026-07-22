import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let color = 'red';
	$$renderer.push(`<div${$.attributes({ ...props }, void 0, void 0, { color })}></div>`);
}
