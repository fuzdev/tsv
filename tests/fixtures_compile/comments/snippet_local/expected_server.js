import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// the name
	let name = 'world';
	function greeting($$renderer) {
		$$renderer.push(`<p>hello world</p>`);
	}
	greeting($$renderer);
}
