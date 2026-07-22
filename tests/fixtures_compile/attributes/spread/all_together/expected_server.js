import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let x = 1;
	let v = '';
	let w = '';
	$$renderer.push(
		`<input${$.attributes({ value: w, ...props }, void 0, { a: x }, { color: v }, 4)}/>`
	);
}
