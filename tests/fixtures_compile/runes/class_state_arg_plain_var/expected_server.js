import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let n = 1;
		class C {
			x = n;
		}
		const c = new C();
		$$renderer.push(`<p>${$.escape(c.x)}</p>`);
	});
}
