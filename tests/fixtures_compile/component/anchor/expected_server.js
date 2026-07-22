import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<div>`);
	Foo($$renderer, {});
	$$renderer.push(`<!----></div>`);
}
