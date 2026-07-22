import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	{
		function header($$renderer) {
			$$renderer.push(`<h1>t</h1>`);
		}
		Foo($$renderer, {
			header,
			children: ($$renderer) => {
				$$renderer.push(`<!---->text`);
			},
			$$slots: { header: true, default: true }
		});
	}
}
