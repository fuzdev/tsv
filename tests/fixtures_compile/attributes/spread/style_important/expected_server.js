import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let v = '';
	$$renderer.push(`<div${$.attributes({ ...props }, void 0, void 0, { c: v })}></div>`);
}
