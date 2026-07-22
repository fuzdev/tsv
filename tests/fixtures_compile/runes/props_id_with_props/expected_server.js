import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	const id = $.props_id($$renderer);
	let { name } = $$props;
	$$renderer.push(`<!---->${$.escape(name)}${$.escape(id)}`);
}
