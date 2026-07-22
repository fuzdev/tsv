import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function failed($$renderer, e) {
		$$renderer.push(`<b>${$.escape(e)}</b>`);
	}
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[!-->`);
		{
			$$renderer.push(`<span>load</span>`);
		}
		$$renderer.push(`<!--]-->`);
	});
}
