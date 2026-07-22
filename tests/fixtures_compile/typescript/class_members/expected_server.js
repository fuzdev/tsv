import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Base {
			tag = 'b';
		}
		class C extends Base {
			kept = 1;
			hidden = 2;
			tag = 'c';
			method(label) {}
		}
		let value = new C().kept;
		$$renderer.push(`<p>${$.escape(value)}</p>`);
	});
}
