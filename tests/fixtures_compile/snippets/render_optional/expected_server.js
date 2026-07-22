import * as $ from 'svelte/internal/server';
function foo($$renderer) {
	$$renderer.push(`<b>s</b>`);
}
export default function Input($$renderer) {
	foo?.($$renderer);
}
