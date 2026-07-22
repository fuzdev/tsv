import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = 1;
	function failed($$renderer, e) {
		$$renderer.push(`<b>1</b>`);
	}
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[-->`);
		{
			function inner($$renderer) {
				$$renderer.push(`<p>1</p>`);
			}
			inner($$renderer);
		}
		$$renderer.push(`<!--]-->`);
	});
}
