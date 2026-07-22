import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function failed($$renderer, e) {
		$$renderer.push(`<b>x</b>`);
	}
	$$renderer.push(`<button>go</button> `);
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[-->`);
		{
			$$renderer.push(`<p>a</p>`);
		}
		$$renderer.push(`<!--]-->`);
	});
}
