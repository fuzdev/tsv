import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	{
		function failed($$renderer, e) {
			$$renderer.push(`<b>x</b>`);
		}
		$$renderer.boundary({ failed }, ($$renderer) => {
			$$renderer.push(`<!--[-->`);
			{
				$$renderer.push(`<p>a</p>`);
			}
			$$renderer.push(`<!--]-->`);
		});
	}
	$$renderer.push(` `);
	{
		function failed($$renderer, e) {
			$$renderer.push(`<i>y</i>`);
		}
		$$renderer.boundary({ failed }, ($$renderer) => {
			$$renderer.push(`<!--[-->`);
			{
				$$renderer.push(`<p>b</p>`);
			}
			$$renderer.push(`<!--]-->`);
		});
	}
}
