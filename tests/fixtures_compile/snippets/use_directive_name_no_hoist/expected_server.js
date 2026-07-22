import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let localAction = () => {};
	function s($$renderer) {
		$$renderer.push(`<div></div>`);
	}
	s($$renderer);
}
