import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Base {
			value = 1;
		}
		class Impl extends Base {
			run() {}
		}
		let a = new Impl().value;
		$$renderer.push(`<p>${$.escape(a)}</p>`);
	});
}
