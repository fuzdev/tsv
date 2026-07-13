import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { a, f } = $$props;
		$$renderer.push(
			`<p>${$.escape(a)}</p> <p>${$.escape(a)}</p> <p>${$.escape(a)}</p> <p>${$.escape(f(a))}</p> <p>${$.html(a)}</p>`
		);
	});
}
