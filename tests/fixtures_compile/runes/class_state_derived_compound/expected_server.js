import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let n = 1;
		const d = $.derived(() => n * 2);
		class C {
			x = d() + 1;
		}
		const c = new C();
		$$renderer.push(`<p>${$.escape(c.x)}</p>`);
	});
}
