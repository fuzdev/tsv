import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class Model {
			a = 1;
			label = 'hi';
			b = 2;
			static kind = 'model';
			total() {
				return this.a + this.b;
			}
		}
		const m = new Model();
		$$renderer.push(`<p>${$.escape(m.label)}: ${$.escape(m.total())}</p>`);
	});
}
