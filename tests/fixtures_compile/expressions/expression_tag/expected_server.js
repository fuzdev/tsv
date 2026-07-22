import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { prop } = $$props;
	$$renderer.push(`<p>${$.escape(prop)}</p>`);
}
