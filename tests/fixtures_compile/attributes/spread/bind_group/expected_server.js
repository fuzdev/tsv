import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let props = {};
	let x = 'a';
	$$renderer.push(
		`<input${$.attributes({ type: 'radio', checked: x === 'a', value: 'a', ...props }, void 0, void 0, void 0, 4)}/>`
	);
}
