import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let active = true;
	$$renderer.push(`<div${$.attributes({ ...props }, void 0, { active })}></div>`);
}
