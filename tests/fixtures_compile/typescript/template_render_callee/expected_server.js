import * as $ from 'svelte/internal/server';
function row($$renderer, x) {
	$$renderer.push(`<p>${$.escape(x)}</p>`);
}
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	row($$renderer, a);
}
