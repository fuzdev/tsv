import * as $ from 'svelte/internal/server';
function a($$renderer) {
	$$renderer.push(`<!---->static`);
}
export default function Input($$renderer) {
	let v = 1;
	function a($$renderer) {
		$$renderer.push(`<!---->1`);
	}
	$$renderer.push(`<div></div> `);
	a($$renderer);
	$$renderer.push(`<!---->`);
}
