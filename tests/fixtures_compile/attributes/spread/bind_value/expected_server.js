import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let w = '';
	$$renderer.push(`<input${$.attributes({ value: w, ...props }, void 0, void 0, void 0, 4)}/>`);
}
