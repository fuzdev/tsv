import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function failed($$renderer, error, reset) {
		$$renderer.push(`<button>${$.escape(error.message)}</button>`);
	}
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[-->`);
		{
			$$renderer.push(`<p>hi</p>`);
		}
		$$renderer.push(`<!--]-->`);
	});
}
