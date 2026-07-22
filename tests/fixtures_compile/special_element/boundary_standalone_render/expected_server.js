import * as $ from 'svelte/internal/server';
function s($$renderer) {
	$$renderer.push(`<p>x</p>`);
}
export default function Input($$renderer) {
	$$renderer.push(`<!--[-->`);
	{
		s($$renderer);
	}
	$$renderer.push(`<!--]-->`);
}
