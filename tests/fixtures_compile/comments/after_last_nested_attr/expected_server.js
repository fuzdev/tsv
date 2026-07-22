import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { href } = $$props;
	$$renderer.push(
		`<a${$.attr(
			'href',
			// dangling
			href
		)}>x</a>`
	);
}
