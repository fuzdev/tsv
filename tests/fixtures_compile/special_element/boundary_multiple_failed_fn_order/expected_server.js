import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function failed($$renderer, e) {
		$$renderer.push(`<i>A</i>`);
	}
	function failed($$renderer, f) {
		$$renderer.push(`<u>B</u>`);
	}
	$$renderer.push(`<b>1</b>`);
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[-->`);
		{
			$$renderer.push(`<p>a</p>`);
		}
		$$renderer.push(`<!--]-->`);
	});
	$$renderer.push(`<b>2</b>`);
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[-->`);
		{
			$$renderer.push(`<p>c</p>`);
		}
		$$renderer.push(`<!--]-->`);
	});
}
