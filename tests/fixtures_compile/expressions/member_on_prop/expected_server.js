import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { a } = $$props;
		$$renderer.push(`<p>${$.escape(a.b)}</p>`);
	});
}
