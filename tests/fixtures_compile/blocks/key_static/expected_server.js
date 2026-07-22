import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { k } = $$props;
	$$renderer.push(`<!---->`);
	{
		$$renderer.push(`<p>content</p>`);
	}
	$$renderer.push(`<!---->`);
}
