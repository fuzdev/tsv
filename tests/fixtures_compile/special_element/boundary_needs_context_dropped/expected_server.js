import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		class C {}
		$$renderer.push(`<!--[!-->`);
		{
			$$renderer.push(`<i>w</i>`);
		}
		$$renderer.push(`<!--]-->`);
	});
}
