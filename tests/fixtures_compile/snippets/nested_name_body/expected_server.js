import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let v = 1;
	function a($$renderer) {
		$$renderer.push(`<!---->1`);
	}
	function a($$renderer) {
		$$renderer.push(`<!---->nested`);
	}
	$$renderer.push(`<div></div> `);
	a($$renderer);
	$$renderer.push(`<!---->`);
}
