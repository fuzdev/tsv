import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let x = 1;
	$$renderer.push(`<input${$.attributes({ ...props }, void 0, void 0, void 0, 4)}/>`);
}
