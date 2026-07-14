import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let x = 1;
	$$renderer.push(`<div${$.attributes({ title: 'a 1 b', ...props })}></div>`);
}
