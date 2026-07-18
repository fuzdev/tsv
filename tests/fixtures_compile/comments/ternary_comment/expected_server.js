import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { n } = $$props;
	const label = n
		? // pick a branch
			'pos'
		: 'neg';
	$$renderer.push(`<p>${$.escape(label)}</p>`);
}
