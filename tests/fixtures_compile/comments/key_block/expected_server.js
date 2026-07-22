import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// keyed
	let k = 1;
	$$renderer.push(`<!---->`);
	{
		$$renderer.push(`<p>hi</p>`);
	}
	$$renderer.push(`<!---->`);
}
