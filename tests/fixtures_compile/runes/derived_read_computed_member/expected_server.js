import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { a, obj } = $$props;
		let d = $.derived(() => a * 2);
		$$renderer.push(`<!---->${$.escape(obj[d()])}`);
	});
}
