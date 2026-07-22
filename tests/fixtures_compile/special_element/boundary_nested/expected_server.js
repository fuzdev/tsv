import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function failed($$renderer, e) {
		$$renderer.push(`<b>out</b>`);
	}
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[-->`);
		{
			function failed($$renderer, f) {
				$$renderer.push(`<i>in</i>`);
			}
			$$renderer.boundary({ failed }, ($$renderer) => {
				$$renderer.push(`<!--[-->`);
				{
					$$renderer.push(`<p>a</p>`);
				}
				$$renderer.push(`<!--]-->`);
			});
		}
		$$renderer.push(`<!--]-->`);
	});
}
