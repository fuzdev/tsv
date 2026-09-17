import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const q = { a: 2, b: 3 };
		// a pattern default
		const { a = 1, b } = q;
		const p = { a, /* after the value */ b };
		class C {
			m() {
				// nested in a method
				return {
					a,
					get g() {
						return 1;
					}
				};
			}
		}
		$$renderer.push(`<p>${$.escape(p.b)}${$.escape(new C().m().a)}</p>`);
	});
}
