import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let arr = { a: 1 };
	$$renderer.push(`<div${$.attributes({ ...$.snapshot(arr) })}></div>`);
}
