import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	{
		function a($$renderer) {
			$$renderer.push(`<b>1</b>`);
		}
		function b($$renderer) {
			$$renderer.push(`<i>2</i>`);
		}
		Foo($$renderer, { a, b, $$slots: { a: true, b: true } });
	}
}
