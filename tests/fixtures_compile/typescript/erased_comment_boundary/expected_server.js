import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	/** A leading comment sits BEFORE the erased region and survives. */
	let { a } = $$props;
	$$renderer.push(`<p>${$.escape(a)}</p>`);
}
