import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	inner($$renderer);
	$$renderer.push(`<!---->`);
	$$renderer.push(`<!--[!-->`);
	{
		function inner($$renderer) {
			$$renderer.push(`<i>q</i>`);
		}
		$$renderer.push(`<!---->x`);
	}
	$$renderer.push(`<!--]-->`);
}
