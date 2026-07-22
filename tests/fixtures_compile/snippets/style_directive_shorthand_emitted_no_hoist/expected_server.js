import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let color = 'red';
	function s($$renderer) {
		$$renderer.push(`<div${$.attr_style('', { color })}>x</div>`);
	}
	s($$renderer);
}
