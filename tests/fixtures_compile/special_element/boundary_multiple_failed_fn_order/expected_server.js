import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<b>1</b>`);
	{
		function failed($$renderer, e) {
			$$renderer.push(`<i>A</i>`);
		}
		$$renderer.boundary({ failed }, ($$renderer) => {
			$$renderer.push(`<!--[-->`);
			{
				$$renderer.push(`<p>a</p>`);
			}
			$$renderer.push(`<!--]-->`);
		});
	}
	$$renderer.push(`<b>2</b>`);
	{
		function failed($$renderer, f) {
			$$renderer.push(`<u>B</u>`);
		}
		$$renderer.boundary({ failed }, ($$renderer) => {
			$$renderer.push(`<!--[-->`);
			{
				$$renderer.push(`<p>c</p>`);
			}
			$$renderer.push(`<!--]-->`);
		});
	}
}
