import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let hidden = true;
	$$renderer.push(`<div${$.attributes({ hidden, ...props })}></div>`);
}
