import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let box = '0 0 10 10';
	$$renderer.push(
		`<svg${$.attributes({ ...props, viewBox: box }, void 0, void 0, void 0, 3)}></svg>`
	);
}
