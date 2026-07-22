import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	{
		function children($$renderer) {
			$$renderer.push(`<p>c</p>`);
		}
		Foo($$renderer, { children, $$slots: { default: true } });
	}
}
